// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { LayoutPanes } from "./LayoutPanes";
import { GRIP, GRIP_HANDLE, PlacingHere, type Placing } from "./paneDrag";
import type { ChatHead } from "./paneHead";
import type { Layout } from "./tabs";

/**
 * **One grip, on each kind of pane** (#5290, spec #5288 stories 2, 12 to 19): a pane's mark in its
 * head is its grip (`ui/grip`), named "Move" and what it moves, a press of it picks the pane up.
 * A one-job program, an empty pane and a view alone in its tab have none; a view in a split has
 * one, at its head's start, as a chat's pane has its persona's. The panes are `LayoutPanes`'s
 * own, with what each holds stubbed.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => <div>session {session}</div>,
}));
vi.mock("./Views", async (real) => ({
  ...(await real<typeof import("./Views")>()),
  ViewPane: () => <div className="view-head">a view</div>,
}));

beforeEach(() => {
  mockIPC(() => null, { shouldMockEvents: true });
});
afterEach(async () => {
  cleanup();
  await new Promise((resolve) => setTimeout(resolve, 0));
  clearMocks();
});

const chat = (pane: number, oneJob = false): Layout => ({
  kind: "pane",
  pane,
  content: {
    kind: "session",
    session: pane,
    chat: `chat ${pane}`,
    ...(oneJob ? { oneJob: {} } : {}),
  },
});
const empty = (pane: number): Layout => ({
  kind: "pane",
  pane,
  content: { kind: "empty", workspace: "ops" },
});
const settings = (pane: number): Layout => ({
  kind: "pane",
  pane,
  content: { kind: "view", view: { from: null, view: "settings", key: "" }, workspace: "ops" },
});
const row = (one: Layout, other: Layout): Layout => ({
  kind: "split",
  direction: "row",
  children: [one, other],
  share: 500,
});

const head = (session: number) =>
  ({
    chat: {
      session,
      name: `chat ${session}`,
      persona: "steward",
      shell: false,
      report: null,
      outcome: null,
      harness: "claude-code",
    },
    sandbox: null,
    wrapping: false,
  }) as unknown as ChatHead;

type Props = Parameters<typeof LayoutPanes>[0];

function propsOf(layout: Layout, alone: boolean): Props {
  return {
    alone,
    plane: "/home/op/project",
    layout,
    tab: 1,
    focused: 1,
    onFocus: vi.fn(),
    offerFor: () => undefined,
    closeOf: () => undefined,
    onPaneDoes: vi.fn(),
    runningIn: () => 0,
    split: { of: () => undefined, moved: vi.fn(), shared: vi.fn() },
    waiting: [],
    asks: [],
    crumbs: {},
    placed: {},
    endedTasks: {},
    endedChats: new Map(),
    unread: new Set(),
    finished: {},
    others: {},
    asked: {},
    byHand: {},
    startNotes: {},
    blocks: {},
    restartsSaid: {},
    heads: head,
    glances: {},
    handedFrom: {},
    workItems: {},
    shells: new Set(),
    restoring: new Set(),
    offers: new Map(),
    memory: {},
    chatName: () => "chat",
    faceOf: () => ({ name: "chat", icon: null }),
    room: { width: 1600, height: 900 },
  } as unknown as Props;
}

/** The panes of `layout`, where a pane can be picked up, and what was picked up. */
function drawn(layout: Layout) {
  // A tab's only pane is drawn alone, as `TabPanes` says (#5318).
  const alone = layout.kind === "pane";
  const pickUp = vi.fn();
  const placing: Placing = { pickUp, gripName: (_tab, pane) => `Move pane ${pane}` };
  render(
    <PlacingHere placing={placing}>
      <LayoutPanes {...propsOf(layout, alone)} />
    </PlacingHere>,
  );
  return { pickUp };
}

const frameOf = (pane: number) =>
  document.querySelector<HTMLElement>(`[data-drop-pane="${pane}"]`) as HTMLElement;
const gripsIn = (pane: number) =>
  frameOf(pane).querySelectorAll('[data-scope="grip"][data-part="root"]');

describe("a pane's grip (#5290)", () => {
  it("is the persona's mark in a chat's head, a control named Move, with the state's badge beside it", async () => {
    const { pickUp } = drawn(row(chat(1), chat(2)));

    const grip = screen.getByRole("button", { name: "Move pane 1" });
    expect(grip).toHaveAttribute("data-scope", "grip");
    expect(grip).toHaveAttribute(GRIP_HANDLE);
    // ⌘⇧M does the same, and a reader is told so after its name.
    expect(grip).toHaveAccessibleDescription(
      /^Drag it, or press it or (⌘⇧M|Ctrl\+Shift\+M) to move it with the keys$/,
    );
    // It stands in the chat mark's `who`, so the badge stays the mark's, beside it.
    const who = grip.closest('[data-scope="chat-mark"][data-part="who"]');
    expect(who).not.toBeNull();
    expect(grip.querySelector(".persona-mark")).not.toBeNull();
    expect(grip.querySelectorAll('[data-part="dot"]')).toHaveLength(6);
    // No separate grip button is drawn: the mark is the grip.
    expect(frameOf(1).querySelectorAll(`[${GRIP_HANDLE}]`)).toHaveLength(1);

    await userEvent.click(grip);
    expect(pickUp).toHaveBeenCalledWith({ kind: "pane", tab: 1, pane: 1 }, grip);
  });

  it("is reached by Tab, and Enter on it picks the pane up", async () => {
    const { pickUp } = drawn(row(chat(1), chat(2)));
    const grip = screen.getByRole("button", { name: "Move pane 2" });
    grip.focus();
    await userEvent.keyboard("{Enter}");
    expect(pickUp).toHaveBeenCalledWith({ kind: "pane", tab: 1, pane: 2 }, grip);
  });

  it("is drawn on no one-job program, whose mark stays and whose head does not drag", () => {
    drawn(row(chat(1), chat(2, true)));

    expect(gripsIn(2)).toHaveLength(0);
    expect(frameOf(2).querySelector(".pane-persona .persona-mark")).not.toBeNull();
    expect(frameOf(2).querySelector(`[${GRIP}]`)).toBeNull();
    expect(gripsIn(1)).toHaveLength(1);
  });

  it("is drawn on no empty pane", () => {
    drawn(row(chat(1), empty(2)));

    expect(gripsIn(2)).toHaveLength(0);
    expect(frameOf(2).querySelector(`[${GRIP}]`)).toBeNull();
  });

  it("is drawn on no view alone in its tab, so nothing stands on its own first row", () => {
    drawn(settings(1));

    expect(gripsIn(1)).toHaveLength(0);
    expect(frameOf(1).querySelector(`[${GRIP}]`)).toBeNull();
    // The head draws nothing over the view at all: no actions, no grip (the Settings case).
    expect(frameOf(1).querySelector(".pane-head")?.children).toHaveLength(0);
  });

  it("is the view's mark at its head's start in a split, as a chat's pane has its persona's", async () => {
    const { pickUp } = drawn(row(chat(1), settings(2)));

    const grip = within(frameOf(2)).getByRole("button", { name: "Move pane 2" });
    const paneHead = frameOf(2).querySelector(".pane-head");
    expect(paneHead).toHaveAttribute(GRIP);
    expect(paneHead?.firstElementChild).toHaveClass("pane-view-grip");
    expect(paneHead?.firstElementChild?.contains(grip)).toBe(true);
    expect(grip.querySelector(".tab-mark")).not.toBeNull();
    fireEvent.click(grip);
    expect(pickUp).toHaveBeenCalledWith({ kind: "pane", tab: 1, pane: 2 }, grip);
  });
});

describe("one grip, where a pane has no mark to stand in (#5537)", () => {
  it("leaves a view's blank head to its own first row: only its grip and its actions take the pointer", () => {
    // The window's own stylesheet, which decides what the pointer lands on.
    const style = document.createElement("style");
    style.textContent = readFileSync(join(process.cwd(), "src", "paneFrame.css"), "utf8");
    document.head.append(style);
    try {
      drawn(row(chat(1), settings(2)));
      const paneHead = frameOf(2).querySelector(".pane-head") as HTMLElement;
      // A press on the head's blank part goes through to the view's first row beneath it.
      expect(getComputedStyle(paneHead).pointerEvents).toBe("none");
      const grip = paneHead.querySelector(".pane-view-grip") as HTMLElement;
      expect(getComputedStyle(grip).pointerEvents).toBe("auto");
      const actions = paneHead.querySelector(".pane-actions") as HTMLElement;
      expect(getComputedStyle(actions).pointerEvents).toBe("auto");
      // A chat's head has no first row under it: all of it takes the pointer, and drags.
      expect(
        getComputedStyle(frameOf(1).querySelector(".pane-head") as HTMLElement).pointerEvents,
      ).not.toBe("none");
    } finally {
      style.remove();
    }
  });

  it("draws no grip on a chat with no row in the Chats list, and its head stays where a drag starts", () => {
    const pickUp = vi.fn();
    const placing: Placing = { pickUp, gripName: (_tab, pane) => `Move pane ${pane}` };
    render(
      <PlacingHere placing={placing}>
        <LayoutPanes
          {...propsOf(row(chat(1), chat(2)), false)}
          heads={(session: number) => (session === 2 ? undefined : head(session))}
        />
      </PlacingHere>,
    );

    expect(gripsIn(1)).toHaveLength(1);
    // No row, so no persona and no state: no mark for a grip to stand in, and none made up.
    expect(gripsIn(2)).toHaveLength(0);
    expect(frameOf(2).querySelector(".persona-mark")).toBeNull();
    expect(frameOf(2).querySelector(".pane-head")).toHaveAttribute(GRIP);
  });
});
