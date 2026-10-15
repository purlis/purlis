// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { ProposedChangeTab } from "./ProposedChangeTab";
import { askOpenProposal, OPEN_PROPOSAL_ASK, proposalView } from "./proposedChangeOpen";
import { answerTaken } from "./asks";
import type { ProposedWhole, Shown } from "./bindings";

/**
 * A proposed change too long for a row opens in its own tab (#1975, PC-7): the change whole, and
 * Apply there, which the app judges again by this tab's rules.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const long = "line\n".repeat(1001);

const whole = (more: Partial<ProposedWhole> = {}): ProposedWhole => ({
  path: "personas/devops/persona.md",
  session: 3,
  before: long,
  after: `${long}more\n`,
  hunks: [{ oldStart: 1, oldLines: 1001, newStart: 1, newLines: 1002 }],
  applies: true,
  why: null,
  ...more,
});

function core(read: ProposedWhole | string) {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "proposed_change") {
      if (typeof read === "string") throw read;
      return read;
    }
    if (cmd === "apply_proposed_change") return "Applied to personas/devops/persona.md.";
    return null;
  });
  return (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args);
}

describe("a proposed change's own tab", () => {
  it("draws the change whole and applies it from the tab once it is on the screen", async () => {
    const sent = core(whole());
    render(<ProposedChangeTab plane={PLANE} id={7} />);
    await screen.findByTestId("merge-viewer", undefined, { timeout: 5000 });
    await userEvent.click(screen.getByRole("button", { name: "Apply" }));
    await waitFor(() =>
      expect(sent("apply_proposed_change")).toEqual([{ plane: PLANE, id: 7, from: "tab" }]),
    );
    expect(await screen.findByTestId("proposal-answered")).toHaveTextContent(
      "Applied to personas/devops/persona.md.",
    );
  });

  it("says why a change is not shown, and offers Dismiss alone", async () => {
    const sent = core(
      whole({
        before: "",
        after: "",
        hunks: [],
        applies: false,
        why: "This change holds what looks like a credential, which the Inbox does not show.",
      }),
    );
    render(<ProposedChangeTab plane={PLANE} id={7} />);
    expect(await screen.findByTestId("proposal-not-shown")).toBeTruthy();
    expect(screen.getAllByRole("button").map((one) => one.textContent)).toEqual(["Dismiss"]);
    await userEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(sent("dismiss_proposed_change")).toEqual([{ plane: PLANE, id: 7 }]));
  });

  it("says so where the proposal waits no more", async () => {
    core("That proposed change waits no more: it was answered already, or its chat closed.");
    render(<ProposedChangeTab plane={PLANE} id={7} />);
    expect(await screen.findByTestId("proposal-gone")).toHaveTextContent("waits no more");
  });
});

describe("Open on a proposed change too long for a row", () => {
  it("asks the window to open its tab, and sends nothing to the app", async () => {
    const sent = core(whole());
    const heard = vi.fn();
    window.addEventListener(OPEN_PROPOSAL_ASK, heard);
    const ask: Shown = {
      session: 3,
      ask: "proposal:7",
      says: "Proposes a change to personas/devops/persona.md, which only you apply",
      options: [
        { id: "open", label: "Open", allows: false },
        { id: "dismiss", label: "Dismiss", allows: false },
      ],
      source: "proposed-change",
      chain: ["steward 3"],
      answer: { via: "proposed-change", id: 7, path: "personas/devops/persona.md" },
      writes: { kind: "not-shown", why: "This change is longer than the Inbox shows." },
    };
    const answered = await answerTaken(PLANE, ask, "open");
    window.removeEventListener(OPEN_PROPOSAL_ASK, heard);
    expect(answered).toEqual({ refused: undefined, live: false, opened: true });
    expect(heard).toHaveBeenCalledTimes(1);
    expect((heard.mock.calls[0][0] as CustomEvent).detail).toEqual({
      plane: PLANE,
      id: 7,
      path: "personas/devops/persona.md",
    });
    expect(sent("apply_proposed_change")).toEqual([]);
    expect(proposalView(7)).toEqual({ from: null, view: "proposed-change", key: "7" });
    expect(typeof askOpenProposal).toBe("function");
  });
});
