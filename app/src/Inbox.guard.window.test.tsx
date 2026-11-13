// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Inbox, MOVED_JUST_NOW } from "./Inbox";
import type { Elsewhere } from "./InboxElsewhere";
import { SETTLE_MS } from "./TaskBlocksNotice";
import { forgetInbox } from "./inboxRules";
import { forgetProjectViews } from "./projectViews";
import { chatGroup, openEveryAnswer } from "./test-inbox";
import type { Shown } from "./bindings";

/**
 * **The press-settle guard, characterised** (#3123, #1693, #1695, #3167): an Allow in the Inbox
 * does nothing when it is pressed within {@link SETTLE_MS} of its row moving or changing under
 * the pointer or the keyboard, and says why. Written against the Inbox as it stood on Radix
 * roving focus (#5946), and held across its move onto the list family: only how a chat's group
 * is found changed (a `rowgroup` of the grid, where it was a `region`), never a case, so whatever
 * draws the rows, the guard decides as it did. Each case is pressed both ways: with the pointer
 * and with Enter on the focused Allow.
 */

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date", "performance"] });
});

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInbox();
  forgetProjectViews();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

/** The person reads what is drawn for longer than an Allow waits. */
const read = () => act(() => vi.advanceTimersByTime(SETTLE_MS + 1));

const PLANE = "/home/dev/plane";

const permission = (
  session: number,
  ask: string,
  chain: string[],
  says = "Run cargo test",
  allow = "Allow",
) =>
  ({
    session,
    ask,
    says,
    options: [
      { id: "allow", label: allow, allows: true },
      { id: "deny", label: "Deny", allows: false },
    ],
    source: "permission",
    chain,
    answer: { via: "hook" },
  }) satisfies Shown;

/** A chat paused at 3 blocks on the same target (T1-36): three answers, the lead first. */
const blockLoop = {
  session: 5,
  ask: "paused:5",
  says: "steward 5 was blocked 3 times on the same thing. Let it keep trying?",
  options: [
    { id: "stop-trying", label: "Tell it to stop trying", allows: false, preselected: true },
    { id: "keep-trying", label: "Let it keep trying", allows: true },
    { id: "stop", label: "Stop chat", allows: false },
  ],
  source: "looping",
  chain: ["steward 5"],
  answer: { via: "paused" },
} satisfies Shown;

/** A core that records what the window sends. */
function core() {
  const calls: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args: args as Record<string, unknown> });
    return null;
  });
  return calls;
}

const answered = (calls: { cmd: string }[]) => calls.some(({ cmd }) => cmd === "answer_ask");

/** Draws the Inbox with every Answer open, and reads it. */
function draw(asks: readonly Shown[], elsewhere?: Elsewhere) {
  const props = { plane: PLANE, asks, onGo: vi.fn(), onLeave: vi.fn(), elsewhere };
  const drawn = render(<Inbox {...props} />);
  openEveryAnswer();
  read();
  return {
    again: (next: readonly Shown[], there: Elsewhere | undefined = elsewhere) => {
      drawn.rerender(<Inbox {...props} asks={next} elsewhere={there} />);
      openEveryAnswer();
    },
  };
}

/** The Allow in a chat's group. */
const allowOf = (chat: string, name = "Allow") =>
  within(chatGroup(chat)).getByRole("button", { name });

/** Presses `way` as the person would: with the pointer, or with Enter on it. */
async function press(way: HTMLElement, by: "pointer" | "keyboard") {
  if (by === "pointer") await userEvent.click(way);
  else {
    act(() => way.focus());
    await userEvent.keyboard("{Enter}");
  }
}

const BOTH = ["pointer", "keyboard"] as const;

describe.each(BOTH)("the press-settle guard, pressed by the %s (#3123)", (by) => {
  it("allows nothing on a row an ask joining above it just moved, and allows once it is read", async () => {
    const calls = core();
    const first = permission(3, "01J0A", ["steward 3"]);
    const later = permission(4, "01J0B", ["steward 4"], "Run cargo build");
    const { again } = draw([first, later]);
    again([first, permission(3, "01J0C", ["steward 3"], "Run cargo fmt"), later]);
    await press(allowOf("steward 4"), by);
    expect(within(chatGroup("steward 4")).getByRole("status")).toHaveTextContent(MOVED_JUST_NOW);
    expect(answered(calls)).toBe(false);

    read();
    await press(allowOf("steward 4"), by);
    expect(answered(calls)).toBe(true);
  });

  it("allows nothing on an ask whose words changed, though it kept its place", async () => {
    const calls = core();
    const ask = permission(3, "01J0A", ["steward 3"]);
    const { again } = draw([ask]);
    again([{ ...ask, says: "Run cargo publish" }]);
    await press(allowOf("steward 3"), by);
    expect(screen.getByRole("status")).toHaveTextContent(MOVED_JUST_NOW);
    expect(answered(calls)).toBe(false);
  });

  it("allows nothing on an answer whose own words changed", async () => {
    const calls = core();
    const ask = permission(3, "01J0A", ["steward 3"]);
    const { again } = draw([ask]);
    again([permission(3, "01J0A", ["steward 3"], "Run cargo test", "Allow always")]);
    await press(allowOf("steward 3", "Allow always"), by);
    expect(screen.getByRole("status")).toHaveTextContent(MOVED_JUST_NOW);
    expect(answered(calls)).toBe(false);
  });

  it("allows nothing just after something above grew, though no row above it changed (#3167)", async () => {
    const calls = core();
    // jsdom lays nothing out: every box stands at `lower` on screen.
    let lower = 0;
    vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(() =>
      DOMRect.fromRect({ x: 0, y: lower, width: 100, height: 30 }),
    );
    draw([
      permission(3, "01J0A", ["steward 3"]),
      permission(4, "01J0B", ["steward 4"], "Run cargo build"),
    ]);
    // Something above grew without drawing this row again: only where its Allow is says so.
    lower = 48;
    await press(allowOf("steward 4"), by);
    expect(within(chatGroup("steward 4")).getByRole("status")).toHaveTextContent(MOVED_JUST_NOW);
    expect(answered(calls)).toBe(false);

    read();
    await press(allowOf("steward 4"), by);
    expect(answered(calls)).toBe(true);
  });

  it("allows nothing pressed just after the ask was drawn, says so, and allows once read", async () => {
    const calls = core();
    render(
      <Inbox
        plane={PLANE}
        asks={[permission(3, "01J0A", ["steward 3"])]}
        onGo={vi.fn()}
        onLeave={vi.fn()}
      />,
    );
    openEveryAnswer();
    await press(screen.getByRole("button", { name: "Allow" }), by);
    expect(screen.getByRole("status")).toHaveTextContent(MOVED_JUST_NOW);
    expect(answered(calls)).toBe(false);

    read();
    await press(screen.getByRole("button", { name: "Allow" }), by);
    expect(answered(calls)).toBe(true);
  });

  // T1-36: a block loop's pause moves a chat held still, so each of its three answers waits for
  // its row to settle, Stop chat as much as Let it keep trying.
  it.each(["Tell it to stop trying", "Let it keep trying", "Stop chat"])(
    "holds %s on a loop pause until its row has settled, and acts once it has",
    async (name) => {
      const calls = core();
      render(<Inbox plane={PLANE} asks={[blockLoop]} onGo={vi.fn()} onLeave={vi.fn()} />);
      openEveryAnswer();
      await press(screen.getByRole("button", { name }), by);
      expect(screen.getByRole("status")).toHaveTextContent(MOVED_JUST_NOW);
      expect(calls.some(({ cmd }) => cmd === "answer_pause")).toBe(false);

      read();
      await press(screen.getByRole("button", { name }), by);
      expect(calls.filter(({ cmd }) => cmd === "answer_pause")).toHaveLength(1);
    },
  );

  it("never holds a Deny", async () => {
    const calls = core();
    const { again } = draw([permission(4, "01J0B", ["steward 4"], "Run cargo build")]);
    again([
      permission(3, "01J0A", ["steward 3"]),
      permission(4, "01J0B", ["steward 4"], "Run cargo build"),
    ]);
    await press(allowOf("steward 4", "Deny"), by);
    expect(answered(calls)).toBe(true);
  });
});

describe("where the keyboard lands after an ask is answered and removed (#3123, #1700)", () => {
  it("lands on the row of the first ask left, never the page", async () => {
    const calls = core();
    const first = permission(3, "01J0A", ["steward 3"]);
    const second = permission(4, "01J0B", ["steward 4"], "Run cargo build");
    const { again } = draw([first, second]);
    await press(allowOf("steward 3"), "keyboard");
    expect(answered(calls)).toBe(true);
    again([second]);
    const row = document.activeElement as HTMLElement;
    expect(row).not.toBe(document.body);
    expect(within(chatGroup("steward 4")).getByText("Run cargo build")).toBeTruthy();
    expect(chatGroup("steward 4").contains(row)).toBe(true);
    // The row itself, not one of its buttons: an answer is never one key away by accident.
    expect(row.tagName).not.toBe("BUTTON");
    expect(row).toHaveTextContent("Run cargo build");
  });
});
