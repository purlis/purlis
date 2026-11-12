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
import { chatGroup } from "./test-inbox";
import { beginTriage, endTriage, landOnFirst, triaging } from "./asksTriage";
import type { Shown } from "./bindings";
import { onAMac } from "./mac";
import type { Snoozing } from "./AsksSnooze";

/**
 * **Keyboard triage** (ADR 0121 decision 9, #6016), against Asks drawn on its own: begun as ⌘⇧I
 * begins it (`beginTriage` from where the keyboard was, then the first ask's lead), the keyboard
 * goes from ask to ask as each is answered, has keys for later, go to chat, snooze and stop, and
 * goes back where it was once Asks is empty or the person leaves. The settle guard still decides
 * every press, and a grant never takes the keyboard as it lands.
 */

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date", "performance"] });
});

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInbox();
  forgetProjectViews();
  endTriage();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

/** The person reads what is drawn for longer than an Allow waits. */
const read = () => act(() => vi.advanceTimersByTime(SETTLE_MS + 1));

const PLANE = "/home/dev/plane";

const permission = (session: number, ask: string, says: string): Shown => ({
  session,
  ask,
  says,
  options: [
    { id: "allow", label: "Allow", allows: true },
    { id: "deny", label: "Deny", allows: false },
  ],
  source: "permission",
  chain: [`steward ${session}`],
  answer: { via: "hook" },
});

/** A chat waiting on the person's reply: answered behind its Answer. */
const question = (session: number): Shown => ({
  session,
  ask: `reply:${session}`,
  says: "Which branch?",
  options: [],
  source: "question",
  chain: [`steward ${session}`],
  answer: { via: "in-its-pane" },
});

function core() {
  const calls: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args: args as Record<string, unknown> });
    return null;
  });
  return calls;
}

const answered = (calls: { cmd: string }[]) => calls.filter(({ cmd }) => cmd === "answer_ask");

type Drawn = {
  /** Where the keyboard was before ⌘⇧I: a chat's terminal, as far as Asks knows. */
  before: HTMLElement;
  onGo: ReturnType<typeof vi.fn>;
  onLeave: ReturnType<typeof vi.fn>;
  onStop: ReturnType<typeof vi.fn>;
  snooze: ReturnType<typeof vi.fn>;
  again: (asks: readonly Shown[], elsewhere?: Elsewhere) => void;
};

/** Draws Asks beside where the keyboard was, and begins triage as ⌘⇧I does. */
function triage(asks: readonly Shown[], elsewhere?: Elsewhere): Drawn {
  const onGo = vi.fn();
  const onLeave = vi.fn();
  const onStop = vi.fn();
  const snooze = vi.fn(async () => undefined);
  const snoozing = { snooze, wake: vi.fn(), snoozed: [] } as unknown as Snoozing;
  const props = { plane: PLANE, onGo, onLeave, onStop, snoozing };
  const drawn = render(
    <>
      <textarea aria-label="Chat steward 1" />
      <Inbox part="asks" {...props} asks={asks} elsewhere={elsewhere} />
    </>,
  );
  const before = screen.getByRole("textbox", { name: "Chat steward 1" });
  act(() => before.focus());
  read();
  act(() => {
    beginTriage(document.activeElement);
    landOnFirst(document.querySelector(".inbox-list") as HTMLElement);
  });
  return {
    before,
    onGo,
    onLeave,
    onStop,
    snooze,
    again: (next, there = elsewhere) =>
      drawn.rerender(
        <>
          <textarea aria-label="Chat steward 1" />
          <Inbox part="asks" {...props} asks={next} elsewhere={there} />
        </>,
      ),
  };
}

/** A triage key, as this keyboard types it: ⌘⌥ and the letter on a Mac, Alt+Shift elsewhere,
 *  read by its place. */
const key = (letter: "L" | "G" | "S" | "X") =>
  userEvent.keyboard(
    onAMac()
      ? `{Meta>}{Alt>}[Key${letter}]{/Alt}{/Meta}`
      : `{Alt>}{Shift>}[Key${letter}]{/Shift}{/Alt}`,
  );

const focused = () => document.activeElement as HTMLElement;

describe("⌘⇧I lands on the first ask (#6016)", () => {
  it("lands on the first ask's Answer where it is answered behind one", () => {
    core();
    triage([question(3), permission(4, "01J0B", "Run cargo build")]);
    expect(focused()).toHaveAccessibleName("Answer steward 3");
  });

  it("lands on a decision's answer that grants nothing, never its Allow (#2980)", () => {
    core();
    triage([permission(3, "01J0A", "Run cargo test")]);
    expect(focused()).toHaveAccessibleName("Deny");
    expect(chatGroup("steward 3").contains(focused())).toBe(true);
  });
});

describe("answering moves the keyboard to the next ask (#6016)", () => {
  it("goes on to the next ask's lead once the one answered is gone", async () => {
    const calls = core();
    const first = permission(3, "01J0A", "Run cargo test");
    const second = permission(4, "01J0B", "Run cargo build");
    const { again } = triage([first, second]);
    await userEvent.keyboard("{Enter}");
    expect(answered(calls)).toHaveLength(1);
    again([second]);
    expect(chatGroup("steward 4").contains(focused())).toBe(true);
    expect(focused()).toHaveAccessibleName("Deny");
  });

  it("refuses an Allow pressed just after the keyboard moved onto its row, as a click is", async () => {
    const calls = core();
    const first = permission(3, "01J0A", "Run cargo test");
    const second = permission(4, "01J0B", "Run cargo build");
    const { again } = triage([first, second]);
    await userEvent.keyboard("{Enter}");
    again([second]);
    // One → to the Allow beside the Deny the keyboard landed on, at once.
    const allow = within(chatGroup("steward 4")).getByRole("button", { name: "Allow" });
    act(() => allow.focus());
    await userEvent.keyboard("{Enter}");
    expect(within(chatGroup("steward 4")).getByRole("status")).toHaveTextContent(MOVED_JUST_NOW);
    expect(answered(calls)).toHaveLength(1);
  });

  it("goes back where the keyboard was before ⌘⇧I once Asks is empty", async () => {
    core();
    const only = permission(3, "01J0A", "Run cargo test");
    const { again, before, onLeave } = triage([only]);
    await userEvent.keyboard("{Enter}");
    again([]);
    expect(focused()).toBe(before);
    expect(onLeave).not.toHaveBeenCalled();
    expect(triaging()).toBe(false);
  });

  it("never goes on into another project's asks, which its own Asks answers (ADR 0121, amended 2026-11-12)", async () => {
    core();
    const elsewhere: Elsewhere = {
      asking: [],
      quiet: [],
      onPress: vi.fn(),
      onOpen: vi.fn(),
      waitingHere: [{ plane: "/home/dev/ops", project: "ops", count: 2 }],
    };
    const only = permission(3, "01J0A", "Run cargo test");
    const { again, before } = triage([only], elsewhere);
    await userEvent.keyboard("{Enter}");
    again([], elsewhere);
    expect(focused()).toBe(before);
    expect(screen.queryByRole("button", { name: /^Go to project/ })).toBeNull();
  });
});

describe("triage's keys (#6016)", () => {
  it("puts an ask off for later: the next ask's lead, and round again past the last", async () => {
    const calls = core();
    triage([question(3), permission(4, "01J0B", "Run cargo build")]);
    await key("L");
    expect(chatGroup("steward 4").contains(focused())).toBe(true);
    await key("L");
    expect(focused()).toHaveAccessibleName("Answer steward 3");
    expect(answered(calls)).toHaveLength(0);
  });

  it("goes to the ask's chat, and triage ends there", async () => {
    core();
    const { onGo } = triage([permission(3, "01J0A", "Run cargo test")]);
    await key("G");
    expect(onGo).toHaveBeenCalledWith(3);
    expect(triaging()).toBe(false);
  });

  it("snoozes the ask's chat", async () => {
    core();
    const { snooze } = triage([permission(3, "01J0A", "Run cargo test")]);
    await key("S");
    expect(snooze).toHaveBeenCalledTimes(1);
  });

  it("stops the ask's chat", async () => {
    core();
    const { onStop } = triage([permission(3, "01J0A", "Run cargo test")]);
    await key("X");
    expect(onStop).toHaveBeenCalledWith(3);
  });

  it("does nothing on a letter alone (I-11)", async () => {
    const calls = core();
    const { onGo, onStop, snooze } = triage([permission(3, "01J0A", "Run cargo test")]);
    const on = focused();
    await userEvent.keyboard("lgsx");
    expect(focused()).toBe(on);
    expect(onGo).not.toHaveBeenCalled();
    expect(onStop).not.toHaveBeenCalled();
    expect(snooze).not.toHaveBeenCalled();
    expect(answered(calls)).toHaveLength(0);
  });

  it("goes back where the keyboard was before ⌘⇧I on Escape", async () => {
    core();
    const { before, onLeave } = triage([permission(3, "01J0A", "Run cargo test")]);
    await userEvent.keyboard("{Escape}");
    expect(focused()).toBe(before);
    expect(onLeave).not.toHaveBeenCalled();
  });
});

/** A sandbox refusal of `target`: identical in several chats, it is one row of Asks (#6009). */
const host = (session: number, target = "api.example.com"): Shown => ({
  session,
  ask: `block:${session}:connect:host:${target}`,
  says: `The sandbox refused ${target}`,
  options: [
    { id: "you", label: "Allow for me on this machine", allows: true },
    { id: "chat", label: "Allow only for this chat", allows: true },
    { id: "keep", label: "Keep blocked", allows: false },
  ],
  source: "sandbox-host",
  chain: [`steward ${session}`],
  answer: {
    via: "sandbox-block",
    shown: { operation: "connect", kind: "host", what: "host", target },
  },
});

describe("identical requests are one stop (#6009, #6016)", () => {
  const sameRow = () => screen.getByRole("row", { name: "2 chats want to reach api.example.com" });

  it("lands on the group's Answer each, never its Allow for all", () => {
    core();
    triage([host(3), host(4), permission(5, "01J0C", "Run cargo build")]);
    expect(sameRow().contains(focused())).toBe(true);
    expect(focused()).toHaveAccessibleName("Answer each: steward 3, steward 4");
    expect(within(sameRow()).getByRole("button", { name: "Allow for all" })).toBeTruthy();
  });

  it("puts the whole group off for later in one step", async () => {
    core();
    triage([host(3), host(4), permission(5, "01J0C", "Run cargo build")]);
    await key("L");
    expect(chatGroup("steward 5").contains(focused())).toBe(true);
    expect(focused()).toHaveAccessibleName("Deny");
  });

  it("walks each chat inside a group opened with Answer each", async () => {
    core();
    triage([host(3), host(4)]);
    await userEvent.keyboard("{Enter}");
    expect(focused()).toHaveAttribute("aria-expanded", "true");
    const group = chatGroup("steward 3, steward 4");
    const rows = within(group)
      .getAllByRole("row")
      .filter((row) => row.classList.contains("inbox-ask"));
    expect(rows).toHaveLength(3);
    await key("L");
    expect(focused()).toHaveAccessibleName("Keep blocked");
    expect(rows[1].contains(focused())).toBe(true);
    await key("L");
    expect(focused()).toHaveAccessibleName("Keep blocked");
    expect(rows[2].contains(focused())).toBe(true);
  });
});
