// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { StaleCloneNotices, staleCloneCause } from "./StaleCloneNotices";
import type { StaleClone } from "./bindings";

/**
 * A workspace's repo left on a chat's branch, or on a moved remote (#1985): one Notice per repo
 * in the Inbox, whose Fix the core may refuse, and which goes once the repo is fixed.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";

const ON_A_CHATS_BRANCH: StaleClone = {
  workspace: "smart-ide",
  repo: "widget",
  branch: { branch: "chat-3", default: "main" },
  remote: null,
};

const BOTH: StaleClone = {
  ...ON_A_CHATS_BRANCH,
  remote: { origin: "github.com/acme/widget", now: "github.com/purlis/widget" },
};

/** A core holding `stale` until a fix it answers with `fixed` (a refusal's words) or clears. */
function core(stale: StaleClone[], fixed?: string) {
  let now = stale;
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "stale_clones") return now;
    if (cmd === "fix_stale_clone") {
      if (fixed !== undefined) throw fixed;
      now = [];
      return "widget is on main.";
    }
    return null;
  });
  return asked;
}

function shown(settle = vi.fn(), dismiss = vi.fn(), dismissed = new Set<string>()) {
  render(
    <StaleCloneNotices plane={PLANE} dismissed={dismissed} dismiss={dismiss} settle={settle} />,
  );
  return { settle, dismiss };
}

describe("a stale repo's Notice", () => {
  it("names the repo, its branch and the branch it should be on, once", async () => {
    core([ON_A_CHATS_BRANCH]);
    shown();

    const notice = await screen.findByRole("status", { name: "widget in smart-ide" });
    expect(notice).toHaveTextContent("widget in smart-ide is on chat-3");
    expect(notice).toHaveTextContent("and not on main");
    expect(screen.getAllByRole("status")).toHaveLength(1);
    expect(screen.getByRole("button", { name: "Switch to main" })).toBeInTheDocument();
  });

  it("is one Notice with both fixes for a repo stale both ways", async () => {
    core([BOTH]);
    shown();

    const notice = await screen.findByRole("status", { name: "widget in smart-ide" });
    expect(notice).toHaveTextContent("the forge says it moved to github.com/purlis/widget");
    expect(screen.getAllByRole("status")).toHaveLength(1);
    expect(screen.getByRole("button", { name: "Use the new address" })).toBeInTheDocument();
  });

  it("asks the core to fix the repo it names, and goes once it is fixed", async () => {
    const asked = core([ON_A_CHATS_BRANCH]);
    shown();

    await userEvent.setup().click(await screen.findByRole("button", { name: "Switch to main" }));

    await waitFor(() => expect(screen.queryByRole("status")).not.toBeInTheDocument());
    expect(asked.find((one) => one.cmd === "fix_stale_clone")?.args).toMatchObject({
      plane: PLANE,
      workspace: "smart-ide",
      repo: "widget",
      fix: "branch",
    });
  });

  it("says what blocks a fix the core refused, and stays", async () => {
    core(
      [ON_A_CHATS_BRANCH],
      "Not switched: the repo has changes. Commit or move them, then fix it again.",
    );
    shown();

    await userEvent.setup().click(await screen.findByRole("button", { name: "Switch to main" }));

    const notice = await screen.findByRole("status", { name: "widget in smart-ide" });
    await waitFor(() => expect(notice).toHaveTextContent("Not switched: the repo has changes."));
  });

  it("settles its dismissals with the repos the core still finds stale", async () => {
    core([ON_A_CHATS_BRANCH]);
    const { settle } = shown();

    await waitFor(() =>
      expect(settle).toHaveBeenCalledWith("stale-clone", [staleCloneCause(ON_A_CHATS_BRANCH)]),
    );
  });

  it("is not drawn once dismissed", async () => {
    const asked = core([ON_A_CHATS_BRANCH]);
    shown(vi.fn(), vi.fn(), new Set([staleCloneCause(ON_A_CHATS_BRANCH)]));

    await waitFor(() => expect(asked.map((one) => one.cmd)).toContain("stale_clones"));
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });
});
