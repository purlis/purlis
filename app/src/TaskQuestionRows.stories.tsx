// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Task rows say who they're waiting on** (#4564, spec #4551): a task's row says where its
 * question stands and since when, read from the asks registry (`QuestionsHere`), as
 * `needs you · 3m` or `asking steward 1 · 1m`. A question with its asking chat draws no hand
 * above it; folded, the row above says `1 asking`, with no red.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import type { Shown } from "./bindings";
import { ChatsSection } from "./ChatsSection";
import { ChatsHere, fixedChats, moved, nothingKnown } from "./chatState";
import { chatsTree, type ListedChat } from "./chatsTree";
import { QuestionsHere } from "./QuestionsHere";
import { untilFocused } from "./stories.testkit";
import type { QuestionsHeld } from "./taskQuestions";
import { panelRoot } from "./ui/panel/Panel";

/** Seconds since 1970, as the registry says when. Fixed back from now so the minutes hold. */
const NOW = Math.floor(Date.now() / 1000);

function listed(session: number, more: Partial<ListedChat> = {}): ListedChat {
  return {
    session,
    name: `steward ${session}`,
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
    ...more,
  };
}

const task = (session: number, name: string, persona: string, parent = 1) =>
  listed(session, {
    name,
    persona,
    parent,
    mode: "task",
    from: "steward 1",
    tab: false,
    report: "owed",
  });

/** The steward, a task waiting on the person, and one whose question its asking chat has. */
const ROWS = [
  listed(1),
  task(2, "developer 2", "developer"),
  { ...task(3, "architect 3", "architect"), asking: "steward 1" },
];

const reached: Shown = {
  session: 2,
  ask: "task-question:2:1",
  says: "Per runner or per project?",
  options: [],
  source: "task-question",
  chain: ["steward 1", "developer 2"],
  answer: { via: "task-question", why: "passed-on", from: "steward 1", note: null },
  since: NOW - 185,
  held_until: null,
  writes: null,
  lines: null,
  last_message: null,
};

const QUESTIONS: QuestionsHeld = {
  asks: [reached],
  with_asking_chat: [{ session: 3, asker: "steward 1", since: NOW - 70 }],
};

/** Each task at work, as the board would say: the questions are the registry's alone. */
const AT_WORK = [2, 3].reduce(
  (known, session, at) =>
    moved(known, {
      plane: "/plane",
      session,
      state: "running",
      needs_you: false,
      queue: [],
      moved_at: at + 1,
      sequence: at + 1,
      reports: [],
      refusals: [],
      children: [],
      needs: null,
      stopped: null,
    }),
  nothingKnown,
);

function RowsInTheirPanel() {
  return (
    <ChatsHere.Provider value={fixedChats(AT_WORK)}>
      <QuestionsHere.Provider value={QUESTIONS}>
        <div
          {...panelRoot("side")}
          aria-label="Navigation"
          role="region"
          style={{ display: "flex", flexDirection: "column", inlineSize: 300, blockSize: 180 }}
        >
          <ChatsSection rows={chatsTree(ROWS)} front={1} onOpen={() => {}} />
        </div>
      </QuestionsHere.Provider>
    </ChatsHere.Provider>
  );
}

const meta = {
  component: RowsInTheirPanel,
} satisfies Meta<typeof RowsInTheirPanel>;

export default meta;
type Story = StoryObj<typeof meta>;

const rowOf = (canvas: HTMLElement, name: string) => {
  const found = within(canvas)
    .getAllByRole("treeitem")
    .find((one) => one.querySelector(".session")?.textContent === name);
  if (found === undefined) throw new Error(`no row is named ${name}`);
  return found;
};

/**
 * **Who each task waits on, and since when**: the task whose question reached you says `needs
 * you · 3m` and wears the hand; the one whose asking chat has it says `asking steward 1 · 1m`,
 * and adds nothing to the row above. The keyboard walks the rows. The picture is the three
 * rows at rest.
 */
export const WhoEachTaskWaitsOn: Story = {
  play: async ({ canvasElement }) => {
    await waitFor(() =>
      expect(
        rowOf(canvasElement, "developer 2").querySelector(".question-waits"),
      ).toHaveTextContent("needs you · 3m"),
    );
    await expect(
      rowOf(canvasElement, "architect 3").querySelector(".question-waits"),
    ).toHaveTextContent("asking steward 1 · 1m");
    // The hand above leads to the task that needs you, never to the one its chat has.
    await expect(
      canvasElement.querySelector("button.rolled-up")?.getAttribute("aria-label"),
    ).toMatch(/^Go to developer 2 below steward 1/);
    const first = rowOf(canvasElement, "steward 1");
    first.focus();
    await untilFocused(() => first, "on the first row");
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(
      () => rowOf(canvasElement, "developer 2"),
      "down to the task that needs you",
    );
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => rowOf(canvasElement, "architect 3"), "down to the task asking");
  },
};

/**
 * **Folded, the row above says how many are asking** (the person's ruling on #4551): `1
 * asking`, in the quiet meta words and no red, beside the hand for the one that needs you.
 * Left folds the focused row. The picture is the folded row.
 */
export const FoldedRowSaysAsking: Story = {
  play: async ({ canvasElement }) => {
    const first = rowOf(canvasElement, "steward 1");
    first.focus();
    await untilFocused(() => first, "on the row above");
    await userEvent.keyboard("{ArrowLeft}");
    await waitFor(() => expect(first).toHaveAttribute("aria-expanded", "false"));
    await expect(first.querySelector(".question-asking")).toHaveTextContent("1 asking");
  },
};
