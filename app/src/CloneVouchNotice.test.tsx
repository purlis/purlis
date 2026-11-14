// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { CloneVouchNotices, cloneVouchCause } from "./CloneVouchNotice";
import type { CloneToVouch } from "./bindings";

/**
 * A clone made outside purlis: one Notice per clone, naming its remote and that purlis did not
 * clone it, whose Trust this clone the core may refuse, and which goes once the clone is trusted.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";

const MINE: CloneToVouch = {
  workspace: "smart-ide",
  folder: "workspaces/smart-ide/repos/widget",
  place: "github.com/acme/widget",
  places: ["github.com/acme/widget"],
  made: null,
};

const SPLIT: CloneToVouch = {
  ...MINE,
  place: null,
  places: ["github.com/acme/widget", "github.com/other/widget"],
};

const LABEL = "Clone made outside purlis";

/** A core listing `clones` until a Trust it answers with `refused` (its words) or clears. */
function core(clones: CloneToVouch[], refused?: string) {
  let now = clones;
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "clones_to_vouch") return now;
    if (cmd === "vouch_clone") {
      if (refused !== undefined) throw refused;
      now = [];
      return "github.com/acme/widget";
    }
    return null;
  });
  return asked;
}

function shown(settle = vi.fn(), dismiss = vi.fn(), dismissed = new Set<string>()) {
  render(
    <CloneVouchNotices plane={PLANE} dismissed={dismissed} dismiss={dismiss} settle={settle} />,
  );
  return { settle, dismiss };
}

describe("a clone made outside purlis", () => {
  it("names the folder, says purlis did not clone it and names its remote", async () => {
    core([MINE]);
    shown();

    const notice = await screen.findByRole("status", { name: LABEL });
    expect(notice).toHaveTextContent(
      "workspaces/smart-ide/repos/widget was not cloned by purlis. Its remote is github.com/acme/widget.",
    );
    expect(notice).toHaveTextContent("can use your GitHub access with it.");
    expect(screen.getByRole("button", { name: "Trust this clone" })).toBeInTheDocument();
  });

  it("trusts the clone at exactly the address it shows, and goes once trusted", async () => {
    const asked = core([MINE]);
    shown();

    await userEvent.setup().click(await screen.findByRole("button", { name: "Trust this clone" }));

    await waitFor(() => expect(screen.queryByRole("status")).not.toBeInTheDocument());
    expect(asked.find((one) => one.cmd === "vouch_clone")?.args).toEqual({
      plane: PLANE,
      folder: "workspaces/smart-ide/repos/widget",
      place: "github.com/acme/widget",
    });
  });

  it("says the core's sentence when it refused, and stays", async () => {
    core(
      [MINE],
      "The remote of workspaces/smart-ide/repos/widget changed since the window showed it, so purlis trusted nothing. Look at it again and choose",
    );
    shown();

    await userEvent.setup().click(await screen.findByRole("button", { name: "Trust this clone" }));

    const notice = await screen.findByRole("status", { name: LABEL });
    await waitFor(() =>
      expect(notice).toHaveTextContent("changed since the window showed it, so purlis trusted"),
    );
    expect(screen.getByRole("button", { name: "Trust this clone" })).toBeInTheDocument();
  });

  it("offers no trust for a remote that names two addresses, and says why", async () => {
    core([SPLIT]);
    shown();

    const notice = await screen.findByRole("status", { name: LABEL });
    expect(notice).toHaveTextContent(
      "names more than one address: github.com/acme/widget and github.com/other/widget. purlis can trust it once origin has one address.",
    );
    expect(screen.queryByRole("button", { name: "Trust this clone" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Dismiss" })).toBeInTheDocument();
  });

  it("dismisses by its clone and remote, and settles with what the core still lists", async () => {
    core([MINE]);
    const { settle, dismiss } = shown();

    await waitFor(() =>
      expect(settle).toHaveBeenCalledWith("clone-vouch", [cloneVouchCause(MINE)]),
    );
    await userEvent.setup().click(screen.getByRole("button", { name: "Dismiss" }));
    expect(dismiss).toHaveBeenCalledWith(cloneVouchCause(MINE));
  });

  it("is not drawn once dismissed", async () => {
    const asked = core([MINE]);
    shown(vi.fn(), vi.fn(), new Set([cloneVouchCause(MINE)]));

    await waitFor(() => expect(asked.map((one) => one.cmd)).toContain("clones_to_vouch"));
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("shows again once the clone's remote changes", () => {
    expect(cloneVouchCause(MINE)).not.toEqual(
      cloneVouchCause({ ...MINE, place: "github.com/someone/else", places: [] }),
    );
  });
});
