// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ProposedPin as Offered } from "./bindings";
import { ProposedPin, ProposedPins } from "./ProposedPin";
import { askReviewPin } from "./proposedPinOpen";

const OFFERED: Offered = {
  workspace: "alpha",
  ask: {
    live: true,
    folder: "backlog-state",
    copied: false,
    name: "Backlog state",
    icon: "chart",
    pinned: false,
  },
  by: "backlog sync",
  command: "purlis skill run backlog-sync",
  cwd: null,
};

const PINNED = {
  id: "x",
  folder: "backlog-state",
  name: "Backlog",
  icon: "rocket",
  missing: null,
  live: true,
  too_large: null,
  fresh: null,
};

afterEach(() => {
  cleanup();
  clearMocks();
});

/** The core, answering `proposed_pin` with `offered`; every command asked, in order. */
function aCore(offered: Offered | string = OFFERED): [string, unknown][] {
  const calls: [string, unknown][] = [];
  mockIPC((cmd, args) => {
    calls.push([cmd, args]);
    if (cmd === "proposed_pin") {
      if (typeof offered === "string") throw offered;
      return offered;
    }
    if (cmd === "pin_proposed") return PINNED;
    return null;
  });
  return calls;
}

describe("A chat's proposed pin, in Pin to workspace (#5963)", () => {
  it("names the chat, the command and who will see it, before anything is pinned", async () => {
    const calls = aCore();
    render(<ProposedPin plane="/x/acme" id={4} onClose={() => {}} />);
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveTextContent("Visible to everyone in acme once the project is saved.");
    expect(dialog).toHaveTextContent("backlog sync proposed this pin. Only you pin it.");
    expect(dialog).toHaveTextContent("Regenerate will run purlis skill run backlog-sync.");
    expect(within(dialog).getByLabelText("Name")).toHaveValue("Backlog state");
    expect(calls.map(([cmd]) => cmd)).toEqual(["proposed_pin"]);
  });

  it("says a LOCAL workspace's pin stays on this machine", async () => {
    aCore({ ...OFFERED, ask: { ...OFFERED.ask, live: false } });
    render(<ProposedPin plane="/x/acme" id={4} onClose={() => {}} />);
    expect(await screen.findByRole("dialog")).toHaveTextContent(
      "alpha is a LOCAL workspace, so this artifact stays on this machine.",
    );
  });

  it("pins only on the person's press, with the name and icon they chose", async () => {
    const calls = aCore();
    const onClose = vi.fn();
    render(<ProposedPin plane="/x/acme" id={4} onClose={onClose} />);
    const dialog = await screen.findByRole("dialog");
    await userEvent.clear(within(dialog).getByLabelText("Name"));
    await userEvent.type(within(dialog).getByLabelText("Name"), "Backlog");
    await userEvent.click(within(dialog).getByRole("radio", { name: "rocket" }));
    expect(calls.some(([cmd]) => cmd === "pin_proposed")).toBe(false);
    await userEvent.click(within(dialog).getByRole("button", { name: "Pin to workspace" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(calls).toContainEqual([
      "pin_proposed",
      { plane: "/x/acme", id: 4, name: "Backlog", icon: "rocket" },
    ]);
  });

  it("pins nothing when the person cancels", async () => {
    const calls = aCore();
    const onClose = vi.fn();
    render(<ProposedPin plane="/x/acme" id={4} onClose={onClose} />);
    const dialog = await screen.findByRole("dialog");
    await userEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(onClose).toHaveBeenCalled();
    expect(calls.map(([cmd]) => cmd)).toEqual(["proposed_pin"]);
  });

  it("says why a proposal can no longer be pinned, with a way out", async () => {
    aCore("This folder's pin changed since the pin was proposed, so nothing was pinned.");
    const onClose = vi.fn();
    render(<ProposedPin plane="/x/acme" id={4} onClose={onClose} />);
    const dialog = await screen.findByRole("alertdialog");
    expect(dialog).toHaveTextContent("pin changed since");
    await userEvent.click(within(dialog).getByRole("button", { name: "Close" }));
    expect(onClose).toHaveBeenCalled();
  });

  it("opens when Review… on its row asks for it in this project, and only then", async () => {
    aCore();
    render(<ProposedPins plane="/x/acme" />);
    act(() => askReviewPin("/x/other", 4));
    expect(screen.queryByRole("dialog")).toBeNull();
    act(() => askReviewPin("/x/acme", 4));
    expect(await screen.findByRole("dialog")).toHaveTextContent("backlog sync proposed this pin");
  });
});
