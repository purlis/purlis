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
import { NoticePaneRow } from "./Notice";
import { QuestionsHere } from "./QuestionsHere";
import { FIRST_LINE, QuizBand, QuizPlaces } from "./QuizBand";
import { forgetThisLaunch } from "./regions";
import { expectFocusInSight, untilFocused } from "./stories.testkit";
import { CLOSE_QUIZ } from "./TaskQuiz";
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

/** The steward's pane: a stand-in for its head, the notices band under it with the quiz first in
 *  it (#5213), and a stand-in for its terminal. As tall as a window's pane, since the top takes at
 *  most about half of it and the quiz shows whole. `minimised` hands the band a bell to fold
 *  into, minimised; without it the band shows all it holds. */
function StewardsPane({ asks, minimised }: { asks: Shown[]; minimised?: boolean }) {
  return (
    <QuizPlaces.Provider
      value={{
        topOf: (task) => (task > 4 ? 4 : undefined),
        openTask: () => {},
        openSettings: () => {},
        answerInInbox: () => undefined,
      }}
    >
      <QuestionsHere.Provider value={{ asks, with_asking_chat: [] }}>
        <div
          className="pane-frame pane-headed"
          style={{ display: "flex", flexDirection: "column", inlineSize: 760, blockSize: 680 }}
        >
          <div className="pane-top">
            <div className="pane-head" data-testid="pane-head">
              <span className="ui-text-meta">Claude Code · smart-ide</span>
            </div>
            <NoticePaneRow
              bell={{
                minimised: minimised === true,
                minimise: () => {},
                expand: () => {},
                onHolds: () => {},
                focus: () => {},
              }}
            >
              <QuizBand plane="/plane" session={4} />
            </NoticePaneRow>
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

/** Whether two parts of the band are drawn apart: neither covers any of the other. */
function apart(one: Element, other: Element): boolean {
  const a = one.getBoundingClientRect();
  const b = other.getBoundingClientRect();
  return a.right <= b.left || b.right <= a.left || a.bottom <= b.top || b.bottom <= a.top;
}

const terminal = (canvas: HTMLElement) => within(canvas).getByLabelText("Terminal of steward 4");
const box = (canvas: HTMLElement) => within(canvas).getByRole("textbox", { name: "Your answer" });

/** Opens the question on screen, a row until then (#5215), and waits for its box to take words. */
const opened = async (canvas: HTMLElement) => {
  const toggle = await within(canvas).findByRole("button", { name: "Show the whole question" });
  toggle.click();
  await waitFor(() => expect(box(canvas)).toBeEnabled());
  toggle.blur();
};

/**
 * **The first question, with the note above it said once** (#5213): a quiet note, never a dialog,
 * saying questions now come here, with Change and its ×. The quiz says who asks, why it reached
 * you, the asking chat's note as its own, and how long it has waited, and closes. Nothing in the
 * band is drawn over another part: the note's ways out, the quiz's title and its close each stand
 * apart. Typing in the terminal carries on while it arrives; Tab walks the note's ways out. The
 * picture is the band at rest.
 */
export const FirstQuestionOffersTheChoice: Story = {
  beforeEach: () => {
    pausedOnItsQuestion();
    setWhereYouAnswer({ answer: "top", offered: false });
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText(FIRST_LINE)).toBeVisible();
    const quiz = canvas.getByRole("group", { name: "developer 7 asks" });
    const title = within(quiz).getByText(/^developer 7 asks/);
    const close = within(quiz).getByRole("button", { name: CLOSE_QUIZ });
    const change = canvas.getByRole("button", { name: "Change" });
    const dismiss = canvas.getByRole("button", { name: "Dismiss" });
    for (const way of [change, dismiss, close]) await expect(apart(way, title)).toBe(true);
    await expect(apart(change, quiz)).toBe(true);
    await expect(apart(dismiss, quiz)).toBe(true);
    await opened(canvasElement);
    await expect(quiz).toHaveTextContent("steward 4 passed this on.");
    await expect(quiz).toHaveTextContent("steward 4's note");
    // It never took the keyboard: whatever had it still has it.
    await expect(box(canvasElement)).not.toHaveFocus();
    change.focus();
    await untilFocused(() => change, "on Change");
    await userEvent.tab();
    await untilFocused(() => dismiss, "on the note's Dismiss");
    expectFocusInSight();
    dismiss.blur();
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
    await expect(canvas.queryByText(FIRST_LINE)).toBeNull();
    await opened(canvasElement);
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
    // Esc folds it from the box, what was typed kept, and lands on its toggle (#5215).
    box(canvasElement).focus();
    await untilFocused(() => box(canvasElement), "back in the answer box");
    await userEvent.keyboard("{Escape}");
    const toggle = canvas.getByRole("button", { name: "Show the whole question" });
    await untilFocused(() => toggle, "on the question's toggle");
    expectFocusInSight();
    // Opened again for the picture, the keyboard put down.
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(box(canvasElement)).toBeVisible());
    toggle.blur();
    await userEvent.clear(terminal(canvasElement));
    terminal(canvasElement).blur();
  },
};

/** A question a task wrote over several lines, as the core gives it: its line ends as themselves. */
const OVER_LINES =
  "Which branch should the fix land on?\n\n- **main**, where the next release is cut\n- the fork the person keeps";

/**
 * **A question over several lines is drawn over its lines** (#5213): each line end the task
 * wrote starts a line, and no mark stands in for one. The picture is the quiz at rest.
 */
export const AQuestionOverSeveralLines: Story = {
  args: { asks: [question(7, "developer 7", NOW - 125, { says: OVER_LINES })] },
  beforeEach: () => {
    pausedOnItsQuestion();
    setWhereYouAnswer({ answer: "top", offered: true });
  },
  play: async ({ canvasElement }) => {
    const quiz = within(canvasElement).getByRole("group", { name: "developer 7 asks" });
    await opened(canvasElement);
    await expect(quiz).not.toHaveTextContent(/⟦U\+|\*\*/);
    // Drawn over its lines: the question's line above its list, each of the list's on its own.
    const line = within(quiz).getByText("Which branch should the fix land on?");
    const [first, second] = within(quiz).getAllByRole("listitem");
    await expect(within(quiz).getByText("main").tagName).toBe("STRONG");
    await expect(first.getBoundingClientRect().top).toBeGreaterThanOrEqual(
      line.getBoundingClientRect().bottom,
    );
    await expect(second.getBoundingClientRect().top).toBeGreaterThanOrEqual(
      first.getBoundingClientRect().bottom,
    );
  },
};

/**
 * **Closing a question here** (#5213): Close puts it away from this pane, unanswered, and the one
 * waiting behind it comes after the pause. The picture is the next question.
 */
export const ClosingOneShowsTheNext: Story = {
  args: { asks: THREE },
  beforeEach: () => {
    pausedOnItsQuestion();
    setWhereYouAnswer({ answer: "top", offered: true });
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const quiz = canvas.getByRole("group", { name: "developer 7 asks" });
    const close = within(quiz).getByRole("button", { name: CLOSE_QUIZ });
    close.focus();
    await untilFocused(() => close, "on the quiz's Close");
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(canvas.queryByRole("group", { name: "developer 7 asks" })).toBeNull(),
    );
    await waitFor(
      () => expect(canvas.getByRole("group", { name: "designer 3 asks" })).toBeVisible(),
      { timeout: 5000 },
    );
    await opened(canvasElement);
  },
};

/**
 * **Folded with the pane's notices** (#5213): minimised, which every pane's notices are by
 * default, the note folds into the pane's bell and the band is one line that names the question.
 * The picture is the band minimised.
 */
export const FoldedWithThePanesNotices: Story = {
  args: { asks: [FIRST], minimised: true },
  beforeEach: () => {
    pausedOnItsQuestion();
    setWhereYouAnswer({ answer: "top", offered: false });
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    // Folded, the band is one line, and the note and the question are in it, unseen (#5215).
    const line = await canvas.findByRole("button", {
      name: /^1 waiting · A task's question\s*·\s*Show$/,
    });
    await waitFor(() => expect(canvas.getByText(FIRST_LINE)).not.toBeVisible());
    await waitFor(() =>
      expect(canvas.queryByRole("group", { name: "developer 7 asks" })).toBeNull(),
    );
    line.focus();
    await untilFocused(() => line, "on the band's line");
    expectFocusInSight();
    line.blur();
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
    await opened(canvasElement);
    await expect(canvas.getByText("steward 4 did not reply in 100 seconds.")).toBeVisible();
    await expect(canvas.queryByText(/'s note/)).toBeNull();
  },
};
