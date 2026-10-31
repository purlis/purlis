// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { StaleCloneNotices, staleCloneCause, useStaleClones } from "./StaleCloneNotices";
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
  remote: {
    origin: "github.com/acme/widget",
    now: "https://github.com/purlis/widget.git",
    pushes_follow: false,
    pushes_kept: [],
  },
};

/** A core holding `stale` until a fix it answers with `fixed` (a refusal's words) or clears. */
function core(stale: StaleClone[], fixed?: string, unread: string[] = []) {
  let now = stale;
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "stale_clones") return { stale: now, unread };
    if (cmd === "fix_stale_clone") {
      if (fixed !== undefined) throw fixed;
      now = [];
      return "widget is on main.";
    }
    return null;
  });
  return asked;
}

/** The Inbox's Notices, from the one reading the title bar's chips share (#3650). */
function Listed({
  settle,
  dismiss,
  dismissed,
}: {
  settle: (family: string, present: readonly string[]) => void;
  dismiss: (cause: string) => void;
  dismissed: ReadonlySet<string>;
}) {
  const clones = useStaleClones({ plane: PLANE, dismissed, dismiss, settle });
  return <StaleCloneNotices clones={clones} />;
}

function shown(settle = vi.fn(), dismiss = vi.fn(), dismissed = new Set<string>()) {
  render(<Listed settle={settle} dismiss={dismiss} dismissed={dismissed} />);
  return { settle, dismiss };
}

describe("a stale repo's Notice", () => {
  it("names the repo, its branch and the branch it should be on, once", async () => {
    core([ON_A_CHATS_BRANCH]);
    shown();

    const notice = await screen.findByRole("status", { name: "widget in smart-ide" });
    expect(notice).toHaveTextContent(
      "widget in smart-ide is on chat-3, not main. A chat that starts in this repo works on chat-3.",
    );
    expect(screen.getAllByRole("status")).toHaveLength(1);
    expect(screen.getByRole("button", { name: "Switch to main" })).toBeInTheDocument();
  });

  it("is one Notice with both fixes for a repo stale both ways", async () => {
    core([BOTH]);
    shown();

    const notice = await screen.findByRole("status", { name: "widget in smart-ide" });
    expect(notice).toHaveTextContent(
      "the forge says it moved to https://github.com/purlis/widget.git",
    );
    expect(screen.getAllByRole("status")).toHaveLength(1);
    expect(screen.getByRole("button", { name: "Use the new address" })).toBeInTheDocument();
  });

  it("says which push addresses the new address leaves as they are", async () => {
    core([
      {
        ...BOTH,
        remote: {
          origin: "github.com/acme/widget",
          now: "https://github.com/purlis/widget.git",
          pushes_follow: false,
          pushes_kept: ["github.com/acme/widget", "gitlab.example/mirror/widget"],
        },
      },
    ]);
    shown();

    const notice = await screen.findByRole("status", { name: "widget in smart-ide" });
    expect(notice).toHaveTextContent(
      "It pushes to github.com/acme/widget and gitlab.example/mirror/widget, which Use the new address leaves as they are.",
    );
  });

  it("says pushes go to the new address too when its push address follows", async () => {
    core([
      {
        ...BOTH,
        remote: {
          origin: "github.com/acme/widget",
          now: "https://github.com/purlis/widget.git",
          pushes_follow: true,
          pushes_kept: [],
        },
      },
    ]);
    shown();

    const notice = await screen.findByRole("status", { name: "widget in smart-ide" });
    expect(notice).toHaveTextContent(
      "redirects the old address. Pushes move to the new address too.",
    );
    expect(notice).not.toHaveTextContent("It pushes to");
  });

  it("says nothing of push addresses where it has none of its own", async () => {
    core([BOTH]);
    shown();

    const notice = await screen.findByRole("status", { name: "widget in smart-ide" });
    expect(notice).not.toHaveTextContent("It pushes to");
    expect(notice).not.toHaveTextContent("Pushes move to the new address too.");
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

  it("sets exactly the address it shows", async () => {
    const asked = core([BOTH]);
    shown();

    await userEvent
      .setup()
      .click(await screen.findByRole("button", { name: "Use the new address" }));

    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "fix_stale_clone")?.args).toMatchObject({
        fix: "remote",
        shown: "https://github.com/purlis/widget.git",
      }),
    );
  });

  it("keys a dismissal by the branch, so another branch later shows again", async () => {
    core([{ ...ON_A_CHATS_BRANCH, branch: { branch: "chat-4", default: "main" } }]);
    shown(vi.fn(), vi.fn(), new Set([staleCloneCause(ON_A_CHATS_BRANCH)]));

    const notice = await screen.findByRole("status", { name: "widget in smart-ide" });
    expect(notice).toHaveTextContent("is on chat-4");
  });

  it("keys a dismissal by each finding, so a later move of the same repo shows again", async () => {
    core([BOTH]);
    shown(vi.fn(), vi.fn(), new Set([staleCloneCause(ON_A_CHATS_BRANCH)]));

    expect(await screen.findByRole("status", { name: "widget in smart-ide" })).toBeInTheDocument();
  });

  it("says which workspaces it could not read", async () => {
    core([], undefined, ["could not read workspace 'beta' (gone)."]);
    shown();

    expect(await screen.findByRole("status")).toHaveTextContent(
      "could not read workspace 'beta' (gone).",
    );
  });
});
