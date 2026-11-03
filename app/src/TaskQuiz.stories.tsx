// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Where you answer, and the top-tab quiz** (#4565, spec #4551, the reviewed prototype on
 * `prototype/task-question-surfaces`): a task question that reached the person, drawn as a quiz
 * at the top of its session's pane, saying why it reached them; and, above the first one, the
 * quiet line that offers the choice once. The pane's terminal is a stand-in box, so a story can
 * show the quiz never takes the keyboard from it.
 */
import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { mockIPC } from "@tauri-apps/api/mocks";
import { expect, userEvent, waitFor, within } from "storybook/test";
import type { Shown } from "./bindings";
import { QuestionsHere } from "./QuestionsHere";
import { QuizBand, QuizPlaces } from "./QuizBand";
import { forgetThisLaunch } from "./regions";
import { expectFocusInSight, untilFocused } from "./stories.testkit";
import { TextArea } from "./ui/field/Field";
import { setWhereYouAnswer } from "./whereYouAnswer";

/** Seconds since 1970, fixed back from now so the minutes hold. */
const NOW = Math.floor(Date.now() / 1000);

function question(session: number, task: string, since: number, more: Partial<Shown> = {}): Shown {
  return {
    session,
    ask: `task-question:${session}:1`,
    says: "The pool limit is per runner in the docs but per project in the code. Which one should win?",
    options: [],
    source: "task-question",
    chain: ["steward 4", task],
    answer: {
      via: "task-question",
      why: "passed-on",
      from: "steward 4",
      note: "I can't tell which the docs meant, and it changes what people are billed.",
    },
    since,
    held_until: null,
    writes: null,
    lines: null,
    last_message: null,
    ...more,
  };
}

const FIRST = question(7, "developer 7", NOW - 125);
const THREE = [
  FIRST,
  question(8, "designer 3", NOW - 70, {
    says: "Should the empty Inbox say when the last question closed, or stay blank?",
    answer: { via: "task-question", why: "no-reply", from: "steward 4", note: null },
  }),
  question(9, "architect 2", NOW - 10),
];

/** Where the terminal's input would be: a box the person types in, to show the quiz leaves it be. */
function TerminalStandIn() {
  const [text, setText] = useState("");
  return <TextArea aria-label="Terminal of steward 4" rows={4} value={text} onChange={setText} />;
}

/** The steward's pane: the band at its top, and a stand-in for its terminal under it. */
function StewardsPane({ asks }: { asks: Shown[] }) {
  return (
    <QuizPlaces.Provider
      value={{
        topOf: (task) => (task > 4 ? 4 : undefined),
        openTask: () => {},
        openSettings: () => {},
      }}
    >
      <QuestionsHere.Provider value={{ asks, with_asking_chat: [] }}>
        <div
          className="pane-frame pane-headed"
          style={{ display: "flex", flexDirection: "column", inlineSize: 760, blockSize: 420 }}
        >
          <div className="pane-top">
            <QuizBand plane="/plane" session={4} />
          </div>
          <div className="pane-body">
            <TerminalStandIn />
          </div>
        </div>
      </QuestionsHere.Provider>
    </QuizPlaces.Provider>
  );
}

/** The pretend core: the task is paused on its question, which an answer is for. */
function pausedOnItsQuestion() {
  forgetThisLaunch();
  mockIPC((cmd, args) => {
    if (cmd === "task_question") {
      const session = (args as { session: number }).session;
      return { task: `task ${session}`, asked: "steward 4", number: 1, question: "Which one?" };
    }
    return null;
  });
}

const meta = {
  component: StewardsPane,
  args: { asks: [FIRST] },
} satisfies Meta<typeof StewardsPane>;

export default meta;
type Story = StoryObj<typeof meta>;

const terminal = (canvas: HTMLElement) => within(canvas).getByLabelText("Terminal of steward 4");
const box = (canvas: HTMLElement) => within(canvas).getByRole("textbox", { name: "Your answer" });

/**
 * **The first question, with the choice offered once above it**: a quiet line, never a dialog,
 * with Answer from each session and Keep this. The quiz says who asks, why it reached you, the
 * asking chat's note as its own, and how long it has waited. Typing in the terminal carries on
 * while it arrives; Tab walks the line's answers. The picture is the band at rest.
 */
export const FirstQuestionOffersTheChoice: Story = {
  beforeEach: () => {
    pausedOnItsQuestion();
    setWhereYouAnswer({ answer: "top", offered: false });
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      canvas.getByText("Questions from tasks come to this tab, so you answer them in one place."),
    ).toBeVisible();
    const quiz = canvas.getByRole("group", { name: "developer 7 asks" });
    await expect(quiz).toHaveTextContent("steward 4 passed this on.");
    await expect(quiz).toHaveTextContent("steward 4's note");
    await waitFor(() => expect(box(canvasElement)).toBeEnabled());
    // It never took the keyboard: whatever had it still has it.
    await expect(box(canvasElement)).not.toHaveFocus();
    const each = canvas.getByRole("button", { name: "Answer from each session" });
    each.focus();
    await untilFocused(() => each, "on Answer from each session");
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Keep this" }), "on Keep this");
    expectFocusInSight();
    canvas.getByRole("button", { name: "Keep this" }).blur();
  },
};

/**
 * **One question at a time, oldest first, with a count of the rest**: `2 more`, said in full to
 * a screen reader. The keyboard reaches the box from the terminal only by the person's own move,
 * and Open task is the quiz's last stop. The picture is the quiz at rest.
 */
export const OneAtATimeWithACount: Story = {
  args: { asks: THREE },
  beforeEach: () => {
    pausedOnItsQuestion();
    setWhereYouAnswer({ answer: "top", offered: true });
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("2 more")).toBeVisible();
    await expect(canvas.queryByText(/Questions from tasks come to this tab/)).toBeNull();
    await waitFor(() => expect(box(canvasElement)).toBeEnabled());
    terminal(canvasElement).focus();
    await userEvent.keyboard("pnpm te");
    await expect(terminal(canvasElement)).toHaveFocus();
    await userEvent.keyboard("st");
    await expect(terminal(canvasElement)).toHaveValue("pnpm test");
    box(canvasElement).focus();
    await untilFocused(() => box(canvasElement), "in the answer box");
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Send" }), "on Send");
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Open task" }), "on Open task");
    expectFocusInSight();
    canvas.getByRole("button", { name: "Open task" }).blur();
    await userEvent.clear(terminal(canvasElement));
    terminal(canvasElement).blur();
  },
};

/**
 * **A question its asking chat did not answer in time**, with no note: the reason says so, and
 * the line says nothing of an asking chat's own words. The picture is the quiz at rest.
 */
export const NoReplyInTime: Story = {
  args: { asks: [THREE[1]] },
  beforeEach: () => {
    pausedOnItsQuestion();
    setWhereYouAnswer({ answer: "top", offered: true });
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("steward 4 did not reply in 100 seconds.")).toBeVisible();
    await expect(canvas.queryByText(/'s note/)).toBeNull();
    await waitFor(() => expect(box(canvasElement)).toBeEnabled());
  },
};
