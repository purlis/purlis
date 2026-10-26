// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Inbox, MOVED_JUST_NOW } from "./Inbox";
import { SETTLE_MS } from "./TaskBlocksNotice";
import { DRAWING } from "./AskWrites";
import { forgetInbox } from "./inboxRules";
import type { Shown } from "./bindings";

/**
 * **An edit's change, in its Inbox row, before its Allow** (#1146): Claude Code's Write, Edit and
 * MultiEdit asks are drawn in the light editor's merge view, and no Allow is drawn before it;
 * one whose change cannot be shown says why and offers none.
 */

afterEach(() => {
  vi.useRealTimers();
  cleanup();
  clearMocks();
  forgetInbox();
});

const PLANE = "/home/dev/plane";

const edit = (writes: Shown["writes"]): Shown => ({
  session: 3,
  ask: "e1",
  says: "Change /work/src/lib.rs",
  options: [
    { id: "allow", label: "Allow", allows: true },
    { id: "deny", label: "Deny", allows: false },
  ],
  source: "permission",
  chain: ["steward 3"],
  answer: { via: "hook" },
  writes,
});

function draw(asks: readonly Shown[], calls: string[] = [], Drawn: typeof Inbox = Inbox) {
  mockIPC((cmd) => {
    calls.push(cmd);
    return null;
  });
  return render(
    <Drawn plane={PLANE} asks={asks} onGo={vi.fn()} onLeave={vi.fn()} onAnswered={vi.fn()} />,
  );
}

const buttons = () => screen.getAllByRole("button").map((button) => button.textContent);

/** The text of each line an editor draws, in order. */
const lines = (root: Element) =>
  [...root.querySelectorAll(".cm-line")].map((line) => line.textContent ?? "");

describe("an edit's change in the Inbox (#1146)", () => {
  it("draws the text replaced beside what is put there, in the merge view, and only then Allow", async () => {
    draw([
      edit({
        kind: "whole",
        path: "/work/src/lib.rs",
        changes: [
          {
            says: "Replaces this text",
            before: "fn one() {}\n",
            after: "fn two() {}\n",
            hunks: [{ oldStart: 1, oldLines: 1, newStart: 1, newLines: 1 }],
          },
        ],
      }),
    ]);
    // The view is its own chunk: while it loads, nothing that lets the edit run is drawn.
    expect(screen.getByText(DRAWING)).toBeTruthy();
    expect(buttons()).toEqual(["Go to chat"]);

    const view = await screen.findByTestId("merge-viewer", undefined, { timeout: 5000 });
    expect(view).toHaveAttribute("aria-label", "Replaces this text, in /work/src/lib.rs");
    await vi.waitFor(() => expect(view.querySelectorAll(".cm-editor")).toHaveLength(2));
    const [before, after] = [...view.querySelectorAll(".cm-editor")];
    expect(lines(before)).toEqual(["fn one() {}", ""]);
    expect(lines(after)).toEqual(["fn two() {}", ""]);
    expect(screen.getByText("Replaces this text")).toBeTruthy();
    expect(buttons()).toEqual(["Allow", "Deny", "Go to chat"]);
  });

  it("counts no Allow pressed as the change is drawn, and one pressed once it was read", async () => {
    vi.useFakeTimers({ toFake: ["Date", "performance"] });
    // A window that has drawn no change yet: the merge view's chunk is still to load.
    vi.resetModules();
    const { Inbox: Fresh } = await import("./Inbox");
    const calls: string[] = [];
    draw(
      [
        edit({
          kind: "whole",
          path: "/work/a.txt",
          changes: [
            {
              says: "Replaces this text",
              before: "one",
              after: "two",
              hunks: [{ oldStart: 1, oldLines: 1, newStart: 1, newLines: 1 }],
            },
          ],
        }),
      ],
      calls,
      Fresh,
    );
    // Long after the row was drawn, the change arrives, and the Allow under it with it.
    act(() => vi.advanceTimersByTime(SETTLE_MS + 1));
    const allow = await screen.findByRole("button", { name: "Allow" }, { timeout: 5000 });

    await userEvent.click(allow);
    expect(screen.getByText(MOVED_JUST_NOW)).toBeTruthy();
    expect(calls).not.toContain("answer_ask");

    act(() => vi.advanceTimersByTime(SETTLE_MS + 1));
    await userEvent.click(allow);
    await vi.waitFor(() => expect(calls).toContain("answer_ask"));
  });

  it("draws each change of several, in the order the tool makes them", async () => {
    draw([
      edit({
        kind: "whole",
        path: "/work/a.txt",
        changes: [
          {
            says: "Writes the whole file, whatever it holds now",
            before: "",
            after: "one\n",
            hunks: [{ oldStart: 0, oldLines: 0, newStart: 1, newLines: 1 }],
          },
          {
            says: "Replaces this text everywhere it appears",
            before: "one",
            after: "two",
            hunks: [{ oldStart: 1, oldLines: 1, newStart: 1, newLines: 1 }],
          },
        ],
      }),
    ]);
    const shown = await screen.findByTestId("ask-writes", undefined, { timeout: 5000 });
    expect(within(shown).getAllByTestId("merge-viewer")).toHaveLength(2);
    expect(
      within(shown)
        .getAllByTestId("merge-viewer")
        .map((view) => view.getAttribute("aria-label")),
    ).toEqual([
      "Writes the whole file, whatever it holds now, in /work/a.txt",
      "Replaces this text everywhere it appears, in /work/a.txt",
    ]);
  });

  it("says why a change is not shown, and offers Deny and its pane, never Allow", () => {
    const why = "This change is not text. Its pane shows it whole: allow it there.";
    draw([edit({ kind: "not-shown", why })]);

    expect(screen.getByTestId("ask-writes-not-shown").textContent).toBe(why);
    expect(screen.queryByTestId("merge-viewer")).toBeNull();
    // Even were an Allow offered with it, none is drawn.
    expect(buttons()).toEqual(["Deny", "Go to chat"]);
  });
});

describe("a proposed change in the Inbox (#1975)", () => {
  const proposal = (writes: Shown["writes"], options: Shown["options"]): Shown => ({
    session: 3,
    ask: "proposal:7",
    says: "Proposes a change to purlis.toml, which only you apply",
    options,
    source: "proposed-change",
    chain: ["steward 3"],
    answer: { via: "proposed-change", id: 7, path: "purlis.toml" },
    writes,
  });
  const both: Shown["options"] = [
    { id: "apply", label: "Apply", allows: true },
    { id: "dismiss", label: "Dismiss", allows: false },
  ];

  it("counts no Apply pressed as the change is drawn, and one pressed once it was read", async () => {
    vi.useFakeTimers({ toFake: ["Date", "performance"] });
    // A window that has drawn no change yet: the merge view's chunk is still to load.
    vi.resetModules();
    const { Inbox: Fresh } = await import("./Inbox");
    const calls: string[] = [];
    draw(
      [
        proposal(
          {
            kind: "whole",
            path: "purlis.toml",
            changes: [
              {
                says: "The file as it is, and as the change would make it",
                before: "[plane]\n",
                after: '[plane]\nmode = "pr"\n',
                hunks: [{ oldStart: 1, oldLines: 1, newStart: 1, newLines: 2 }],
              },
            ],
          },
          both,
        ),
      ],
      calls,
      Fresh,
    );
    act(() => vi.advanceTimersByTime(SETTLE_MS + 1));
    const apply = await screen.findByRole("button", { name: "Apply" }, { timeout: 5000 });
    await userEvent.click(apply);
    expect(screen.getByText(MOVED_JUST_NOW)).toBeTruthy();
    expect(calls).not.toContain("apply_proposed_change");

    act(() => vi.advanceTimersByTime(SETTLE_MS + 1));
    await userEvent.click(apply);
    await vi.waitFor(() => expect(calls).toContain("apply_proposed_change"));
  });

  it("never offers Apply beside a change it does not show, whatever the ask lists", () => {
    draw([
      proposal({ kind: "not-shown", why: "This change holds what looks like a credential." }, both),
    ]);
    expect(buttons()).toEqual(["Dismiss", "Go to chat"]);
  });
});
