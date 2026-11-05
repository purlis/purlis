// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Several questions at once** (#4566, spec #4551, the reviewed prototype on
 * `prototype/task-question-surfaces`, screenshots 10 to 15 and the closing line): one quiz at a
 * time with a pause before the next, the line a quiz leaves when an agent answered it first and
 * the person's later word, a question the window cannot draw, and the waiting strip and a quiz
 * stacked at the top of one pane. The quiz is in the pane's notices band, under the head
 * (#5213); the pane's terminal and head are stand-ins.
 */
import { useEffect, useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { mockIPC } from "@tauri-apps/api/mocks";
import { expect, userEvent, waitFor, within } from "storybook/test";
import type { Shown } from "./bindings";
import { NoticePaneRow } from "./Notice";
import { QuestionsHere } from "./QuestionsHere";
import { QuizBand, QuizPlaces } from "./QuizBand";
import { forgetThisLaunch } from "./regions";
import { expectFocusInSight, pictureNow, untilFocused } from "./stories.testkit";
import { TextArea } from "./ui/field/Field";
import { WaitingStrip } from "./WaitingStrip";
import { setWhereYouAnswer } from "./whereYouAnswer";

/** Seconds since 1970, fixed back from now so the minutes hold. */
const NOW = Math.floor(Date.now() / 1000);

function question(session: number, task: string, since: number, more: Partial<Shown> = {}): Shown {
  return {
    session,
    ask: `task-question:${session}:${session}`,
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
const SECOND = question(8, "designer 3", NOW - 70, {
  says: "Should the empty Inbox say when the last question closed, or stay blank?",
  answer: { via: "task-question", why: "no-reply", from: "steward 4", note: null },
});
const THIRD = question(9, "architect 2", NOW - 10);

/** Task 7 stopped on a prompt in its own terminal, which the window cannot draw. */
const PROMPT: Shown = {
  session: 7,
  ask: "terminal:7",
  says: "Waiting in its terminal: a question",
  options: [],
  source: "terminal",
  chain: ["steward 4", "developer 7"],
  answer: { via: "in-its-pane" },
  since: null,
};

/** Steward 4 itself waits on the person: what its strip says. */
const STEWARD_WAITS: Shown = {
  session: 4,
  ask: "question:4",
  says:
    "The release branch is cut. Shall I tag it now and start the pool rollout, or wait for " +
    "the three tasks above to report first?",
  options: [],
  source: "question",
  chain: ["steward 4"],
  answer: { via: "in-its-pane" },
};

/** What steward 4 answered developer 7, as the core shows it. */
const STEWARD_SAID = {
  task: "developer 7",
  from: "steward 4",
  said: "Per project. The code is what customers are billed by.",
  question: "Which one?",
};

/** What the pretend core is asked to move the list to: the asks once something closed. */
const MOVE = "purlis-story:asks-move";

/** Where the terminal's input would be: a box the person types in. */
function TerminalStandIn() {
  const [text, setText] = useState("");
  return <TextArea aria-label="Terminal of steward 4" rows={4} value={text} onChange={setText} />;
}

/**
 * The steward's pane: the strip at its top, a stand-in for its head, the notices band under it
 * with the quiz first in it, and its terminal. The asks become `after` when the pretend core says so ({@link MOVE}).
 */
function StewardsPane({
  asks,
  after,
  strip = false,
  pauseMs,
}: {
  asks: Shown[];
  after?: Shown[];
  strip?: boolean;
  /** How long the pause before the next quiz lasts, where a story pictures it. */
  pauseMs?: number;
}) {
  const [now, setNow] = useState(asks);
  useEffect(() => {
    const move = () => after !== undefined && setNow(after);
    window.addEventListener(MOVE, move);
    return () => window.removeEventListener(MOVE, move);
  }, [after]);
  return (
    <QuizPlaces.Provider
      value={{
        topOf: (task) => (task > 4 ? 4 : undefined),
        openTask: () => {},
        openSettings: () => {},
        answerInInbox: () => undefined,
      }}
    >
      <QuestionsHere.Provider value={{ asks: now, with_asking_chat: [] }}>
        <div
          className="pane-frame pane-headed"
          style={{ display: "flex", flexDirection: "column", inlineSize: 760, blockSize: 680 }}
        >
          <div className="pane-top" data-testid="pane-top">
            {strip && <WaitingStrip session={4} waiting={now} />}
            <div className="pane-head" data-testid="pane-head">
              <span className="ui-text-meta">Claude Code · smart-ide</span>
            </div>
            <NoticePaneRow>
              <QuizBand plane="/plane" session={4} pauseMs={pauseMs} />
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

/** The pretend core: each task is paused on its question; an answer moves the list on. As the
 *  core does, it says no chat answered a question the person answered. */
function theCore() {
  forgetThisLaunch();
  setWhereYouAnswer({ answer: "top", offered: true });
  const theirs = new Set<number | undefined>();
  mockIPC((cmd, args) => {
    const session = (args as { session?: number } | undefined)?.session;
    if (cmd === "task_question")
      return {
        task: `task ${session}`,
        asked: "steward 4",
        number: session,
        question: "Which one?",
      };
    if (cmd === "answer_task_question") {
      theirs.add(session);
      window.dispatchEvent(new Event(MOVE));
      return null;
    }
    if (cmd === "task_question_answered") return theirs.has(session) ? null : STEWARD_SAID;
    return null;
  });
}

/** An asking chat answers first: the list moves on with no answer of the person's. */
const theAgentAnswers = () => window.dispatchEvent(new Event(MOVE));

const meta = {
  component: StewardsPane,
  args: { asks: [FIRST] },
  beforeEach: theCore,
} satisfies Meta<typeof StewardsPane>;

export default meta;
type Story = StoryObj<typeof meta>;

const box = (canvas: HTMLElement) => within(canvas).getByRole("textbox", { name: "Your answer" });

/** Opens the question on screen, a row until then (#5215), and waits for its box to take words. */
const opened = async (canvas: HTMLElement) => {
  const toggle = await within(canvas).findByRole("button", { name: "Show the whole question" });
  toggle.click();
  await waitFor(() => expect(box(canvas)).toBeEnabled());
  toggle.blur();
};

/**
 * **The pause before the next quiz** (story 33): the person answers, and for about a second the
 * quiz's place says where the answer went, at the quiz's height, so a second click lands on
 * nothing. Then the next comes. The picture is the pause, held here past its second so the
 * picture always catches it (the next quiz coming is `QuizBand.test.tsx`'s).
 */
export const PausesBeforeTheNext: Story = {
  args: { asks: [FIRST, SECOND, THIRD], after: [SECOND, THIRD], pauseMs: 600_000 },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await opened(canvasElement);
    box(canvasElement).focus();
    await untilFocused(() => box(canvasElement), "in the answer box");
    await userEvent.keyboard("Per project{Enter}");
    const sent = await canvas.findByText("Sent to developer 7.");
    await waitFor(() => expect(sent).toBeVisible());
    await expect(canvas.getByText("The next question comes in a moment.")).toBeVisible();
    await expect(canvas.queryByRole("button", { name: "Send" })).toBeNull();
    // Nothing of the next quiz is drawn under the pointer while the pause holds.
    await expect(canvas.queryByRole("group", { name: "designer 3 asks" })).toBeNull();
    await pictureNow();
  },
};

/**
 * **An asking chat answered first** (story 36): the quiz closes quietly, with no error, and
 * leaves one line saying who said what, with Go to task and Say otherwise. It stays until
 * dismissed. Tab walks its ways out. The picture is the line at rest.
 */
export const AnAgentAnsweredFirst: Story = {
  args: { asks: [FIRST], after: [] },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await opened(canvasElement);
    theAgentAnswers();
    const line = await canvas.findByText(
      'steward 4 told developer 7: "Per project. The code is what customers are billed by."',
    );
    // The band shows what it holds once it has read its stack again (`NoticePaneRow`).
    await waitFor(() => expect(line).toBeVisible());
    await expect(canvas.queryByRole("group", { name: "developer 7 asks" })).toBeNull();
    const go = canvas.getByRole("button", { name: "Go to task" });
    go.focus();
    await untilFocused(() => go, "on Go to task");
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Say otherwise…" }),
      "on Say otherwise",
    );
    expectFocusInSight();
    canvas.getByRole("button", { name: "Say otherwise…" }).blur();
  },
};

/**
 * **Say otherwise, open** (story 37): the person's own press opens a box under the line, which
 * takes the keyboard, and says the task is told this replaces the chat's answer. The picture is
 * the box with a word typed.
 */
export const SayOtherwise: Story = {
  args: { asks: [FIRST], after: [] },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await opened(canvasElement);
    theAgentAnswers();
    const say = await canvas.findByRole("button", { name: "Say otherwise…" });
    say.focus();
    await untilFocused(() => say, "on Say otherwise");
    await userEvent.keyboard("{Enter}");
    const instead = () => canvas.getByRole("textbox", { name: "What you want instead" });
    await untilFocused(instead, "in the box for the person's word");
    await userEvent.keyboard("Per runner, and say so in the docs");
    await expect(
      canvas.getByText("developer 7 is told this replaces steward 4's answer."),
    ).toBeVisible();
    expectFocusInSight();
    instead().blur();
  },
};

/**
 * **What the person typed survives the chat's answer**: the line says the chat answered first
 * and keeps the box open with their words, to send as their word. The picture is that line.
 */
export const WhatYouTypedIsKept: Story = {
  args: { asks: [FIRST], after: [] },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await opened(canvasElement);
    box(canvasElement).focus();
    await untilFocused(() => box(canvasElement), "in the answer box");
    await userEvent.keyboard("Say when it closed, and keep it to one line");
    box(canvasElement).blur();
    theAgentAnswers();
    const first = await canvas.findByText(
      'steward 4 answered first: "Per project. The code is what customers are billed by." What you typed is still in the box.',
    );
    await waitFor(() => expect(first).toBeVisible());
    await expect(canvas.getByRole("textbox", { name: "What you want instead" })).toHaveValue(
      "Say when it closed, and keep it to one line",
    );
    const send = canvas.getByRole("button", { name: "Send as your word" });
    send.focus();
    await untilFocused(() => send, "on Send as your word");
    expectFocusInSight();
    send.blur();
  },
};

/**
 * **The person's word sent**: the line says the task was told it replaces the chat's answer,
 * with Dismiss. The picture is that line.
 */
export const YourWordReplacesTheChats: Story = {
  args: { asks: [FIRST], after: [] },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await opened(canvasElement);
    theAgentAnswers();
    await userEvent.click(await canvas.findByRole("button", { name: "Say otherwise…" }));
    await userEvent.type(
      canvas.getByRole("textbox", { name: "What you want instead" }),
      "Per runner",
    );
    await userEvent.click(canvas.getByRole("button", { name: "Send" }));
    const told = await canvas.findByText(
      "developer 7 was told your word replaces steward 4's answer.",
    );
    await waitFor(() => expect(told).toBeVisible());
    const dismiss = canvas.getByRole("button", { name: "Dismiss" });
    dismiss.focus();
    await untilFocused(() => dismiss, "on Dismiss");
    expectFocusInSight();
    dismiss.blur();
  },
};

/**
 * **A question the window cannot draw** (story 62): a task stopped on a prompt in its own
 * terminal takes the same place in the queue, as a line linking to its pane, with the count of
 * the rest. The picture is the line.
 */
export const OneTheWindowCannotDraw: Story = {
  args: { asks: [PROMPT, SECOND] },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    // The question whose age is known comes first; the prompt waits behind it.
    await expect(canvas.getByRole("group", { name: "designer 3 asks" })).toBeVisible();
    await expect(canvas.getByText("1 more")).toBeVisible();
  },
};

/** The line alone, at the head of the queue. */
export const OneTheWindowCannotDrawAlone: Story = {
  args: { asks: [PROMPT] },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      canvas.getByText(
        "developer 7 asks something this tab cannot draw. Answer it in the task's pane.",
      ),
    ).toBeVisible();
    const open = canvas.getByRole("button", { name: "Open task" });
    open.focus();
    await untilFocused(() => open, "on Open task");
    expectFocusInSight();
    open.blur();
  },
};

/**
 * **The waiting strip and a quiz, stacked at the top of one pane** (from the rebase of #4955 onto
 * #4949): the strip first, then the pane's head, then the band with the quiz in it (#5213).
 * Together they stay within the top's cap, about half the pane, and nothing covers the head; Send
 * stays reachable, the band scrolling where it must. The picture is the three at rest.
 */
export const StripAndQuizStacked: Story = {
  args: { asks: [STEWARD_WAITS, FIRST, SECOND], strip: true },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("group", { name: "Waiting on you" })).toBeVisible();
    await expect(canvas.getByRole("group", { name: "developer 7 asks" })).toBeVisible();
    const top = canvas.getByTestId("pane-top").getBoundingClientRect();
    const head = canvas.getByTestId("pane-head").getBoundingClientRect();
    const band = canvas.getByRole("region", { name: "Pane notices" }).getBoundingClientRect();
    const pane = canvasElement.querySelector(".pane-frame")?.getBoundingClientRect();
    // Within the top's cap, and the band under the head, whole, inside the top.
    await expect(top.height).toBeLessThanOrEqual((pane?.height ?? 0) * 0.46 + 1);
    await expect(head.height).toBeGreaterThan(0);
    await expect(head.bottom).toBeLessThanOrEqual(band.top + 0.5);
    await expect(band.bottom).toBeLessThanOrEqual(top.bottom + 0.5);
    await opened(canvasElement);
    const send = canvas.getByRole("button", { name: "Send" });
    send.focus();
    await untilFocused(() => send, "on Send");
    expectFocusInSight();
    send.blur();
  },
};
