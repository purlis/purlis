// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A chat's persona with its state on it, one row per state** (#3342): the persona's mark leads
 * each row of the Chats view, and a chat that asks for the person or is at work wears its state's
 * own mark on the mark's corner, about 10px, its shape as well as its colour (`shownState.ts`). A
 * chat at rest wears none. A chat with no persona wears the terminal's mark.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import { ChatsSection } from "./ChatsSection";
import { ChatsHere, fixedChats, nothingKnown, type ChatStates } from "./chatState";
import { chatsTree, type ListedChat } from "./chatsTree";
import { panelRoot } from "./ui/panel/Panel";

/** One chat, a top-level one open in a tab, as the core lists it. */
function listed(session: number, name: string, over: Partial<ListedChat> = {}): ListedChat {
  return {
    session,
    name,
    persona: "steward",
    workspace: "smart-ide",
    shell: false,
    parent: null,
    mode: null,
    from: null,
    tab: true,
    branch: null,
    report: null,
    outcome: null,
    asking: null,
    harness: "Claude Code",
    ...over,
  };
}

/** One row per state the badge tells, then the states at rest, then a terminal at work. */
const CHATS: ListedChat[] = [
  listed(1, "working"),
  listed(2, "needs you"),
  listed(3, "asking", { report: "owed", asking: "steward 1" }),
  listed(4, "failed"),
  listed(5, "unreported", { report: "failed" }),
  listed(6, "done"),
  listed(7, "idle"),
  listed(8, "terminal", { persona: null }),
];

/** What the board says of each. */
const STATES: ChatStates = {
  ...nothingKnown,
  bySession: {
    1: "running",
    2: "waiting",
    3: "running",
    4: "failed",
    6: "done",
    7: "waiting",
    8: "running",
  },
  needsYou: [2],
};

/** The Chats view in its side panel, as the window's slot draws it. */
function OneRowPerState() {
  return (
    <ChatsHere.Provider value={fixedChats(STATES)}>
      <div
        {...panelRoot("side")}
        aria-label="Navigation"
        role="region"
        style={{ display: "flex", flexDirection: "column", inlineSize: 260, blockSize: 420 }}
      >
        <ChatsSection rows={chatsTree(CHATS)} front={1} onOpen={() => {}} />
      </div>
    </ChatsHere.Provider>
  );
}

const meta = { component: OneRowPerState } satisfies Meta<typeof OneRowPerState>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The badge is drawn for the states that ask or are at work, its mark whole; none at rest. */
export const OneRowPerStateOfAChat: Story = {
  play: async ({ canvasElement }) => {
    const badgeOf = (name: string) => {
      const row = [...canvasElement.querySelectorAll<HTMLElement>('[role="treeitem"]')].find(
        (one) => one.querySelector(".session")?.textContent === name,
      );
      return row?.querySelector<HTMLElement>(".chat-persona > .shown-state") ?? null;
    };
    // About 10px: two steps of the theme's space, measured as the theme in force draws them.
    const probe = document.createElement("div");
    probe.style.inlineSize = "calc(var(--space-4) + var(--space-1))";
    canvasElement.append(probe);
    const ten = Math.round(probe.getBoundingClientRect().width);
    probe.remove();
    for (const name of ["working", "needs you", "asking", "failed", "unreported", "terminal"]) {
      const badge = badgeOf(name);
      await expect(badge, name).not.toBeNull();
      await expect(getComputedStyle(badge as HTMLElement).display, name).not.toBe("none");
      // Its shape is drawn: a state is told without its colour.
      const glyph = (badge as HTMLElement).querySelector("svg");
      await expect(glyph, name).not.toBeNull();
      await expect(Math.round((glyph as SVGElement).getBoundingClientRect().width), name).toBe(ten);
    }
    for (const name of ["done", "idle"]) {
      const badge = badgeOf(name);
      await expect(getComputedStyle(badge as HTMLElement).display, name).toBe("none");
    }
  },
};
