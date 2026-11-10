// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A chat's persona with its state on it, one row per state** (#3342): the persona's mark leads
 * each row of the Chats view, and a chat wears its state's own mark on the mark's corner, about
 * 10px, its shape as well as its colour (`ui/chat-mark`). A
 * chat with no persona wears the terminal's mark.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import { ChatsSection } from "./ChatsSection";
import { ChatsHere, fixedChats, nothingKnown, type ChatStates } from "./chatState";
import { chatsTree, type ListedChat } from "./chatsTree";
import { QuestionsHere } from "./QuestionsHere";
import type { QuestionsHeld } from "./taskQuestions";
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
  listed(3, "asking", { report: "owed" }),
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

/** Chat 3's question, with its asking chat: the asks registry's, which is where a row reads
 *  that it is asking (#4564). A minute back, so the row's time holds. */
const QUESTIONS: QuestionsHeld = {
  asks: [],
  with_asking_chat: [{ session: 3, asker: "steward 1", since: Math.floor(Date.now() / 1000) - 60 }],
};

/** The Chats view in its side panel, as the window's slot draws it. */
function OneRowPerState() {
  return (
    <ChatsHere.Provider value={fixedChats(STATES)}>
      <QuestionsHere.Provider value={QUESTIONS}>
        <div
          {...panelRoot("side")}
          aria-label="Navigation"
          role="region"
          style={{ display: "flex", flexDirection: "column", inlineSize: 260, blockSize: 420 }}
        >
          <ChatsSection rows={chatsTree(CHATS)} front={1} onOpen={() => {}} />
        </div>
      </QuestionsHere.Provider>
    </ChatsHere.Provider>
  );
}

const meta = { component: OneRowPerState } satisfies Meta<typeof OneRowPerState>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Every state wears its badge, its glyph whole, and a chat nothing is known of wears none. */
export const OneRowPerStateOfAChat: Story = {
  play: async ({ canvasElement }) => {
    const rootOf = (name: string) =>
      [...canvasElement.querySelectorAll<HTMLElement>('[role="row"]')]
        .find((one) => one.querySelector(".session")?.textContent === name)
        ?.querySelector<HTMLElement>('[data-scope="chat-mark"][data-part="root"]') ?? null;
    // About 10px: two steps of the theme's space, measured as the theme in force draws them.
    const probe = document.createElement("div");
    probe.style.inlineSize = "calc(var(--space-4) + var(--space-1))";
    canvasElement.append(probe);
    const ten = Math.round(probe.getBoundingClientRect().width);
    probe.remove();
    for (const [name, state] of [
      ["working", "working"],
      ["needs you", "needs"],
      ["asking", "waiting"],
      ["failed", "failed"],
      ["unreported", "failed"],
      ["done", "ended"],
      ["idle", "idle"],
      ["terminal", "working"],
    ] as const) {
      const root = rootOf(name);
      await expect(root, name).toHaveAttribute("data-variant", state);
      // Its shape is drawn: a state is told without its colour.
      const glyph = root?.querySelector('[data-part="badge"] svg') ?? null;
      await expect(glyph, name).not.toBeNull();
      await expect(Math.round((glyph as SVGElement).getBoundingClientRect().width), name).toBe(ten);
    }
  },
};
