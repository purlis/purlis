// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Shown } from "./bindings";
import { QuestionsHere } from "./QuestionsHere";
import { NoticePaneRow } from "./Notice";
import { PAUSE_MS, QuizBand, QuizPlaces } from "./QuizBand";
import { forgetThisLaunch } from "./regions";
import { setWhereYouAnswer, whereYouAnswer } from "./whereYouAnswer";

/**
 * **Where a pane draws a task's question** (#4565): the quiz at the top of the pane the setting
 * says, the line that offers the choice once, and what changing it does. Steward 4 (chat 4) has
 * a tab; its task developer 7 (chat 7) asks.
 */

const PLANE = "/home/dev/plane";

const QUESTION: Shown = {
  session: 7,
  ask: "task-question:7:1",
  says: "Which one should win?",
  options: [],
  source: "task-question",
  chain: ["steward 4", "developer 7"],
  answer: { via: "task-question", why: "no-reply", from: "steward 4", note: null },
  since: Math.floor(Date.now() / 1000),
};

const openTask = vi.fn();
const openSettings = vi.fn();
const answerInInbox = vi.fn();

/** Tasks 7 and 8 are at home in steward 4's tab (chat 4); task 21 in talk 2's (chat 20). */
const topOf = (task: number) => (task === 7 || task === 8 ? 4 : task === 21 ? 20 : undefined);

/** One pane's band per session, the quiz band first in it as a chat's pane draws it, the
 *  band minimised or expanded by its bell (`minimised`). */
function Pane({
  session,
  asks,
  pauseMs,
  minimised = false,
}: {
  session: number | number[];
  asks: Shown[];
  pauseMs?: number;
  minimised?: boolean;
}) {
  const bell = {
    minimised,
    minimise: () => {},
    expand: () => {},
    onHolds: () => {},
    focus: () => {},
  };
  return (
    <QuizPlaces.Provider value={{ topOf, openTask, openSettings, answerInInbox }}>
      <QuestionsHere.Provider value={{ asks, with_asking_chat: [] }}>
        {(Array.isArray(session) ? session : [session]).map((one) => (
          <div key={one} data-testid={`pane-${one}`}>
            <NoticePaneRow bell={bell}>
              <QuizBand plane={PLANE} session={one} pauseMs={pauseMs} />
            </NoticePaneRow>
          </div>
        ))}
      </QuestionsHere.Provider>
    </QuizPlaces.Provider>
  );
}

/** What the pretend core was sent to answer, and what it says an asking chat answered. */
let sent: Record<string, unknown>[] = [];
let chatAnswered: unknown = null;
/** Set, the pretend core holds its answer to a send until it is called. */
let holdAnswer: Promise<void> | undefined;

beforeEach(() => {
  forgetThisLaunch();
  sent = [];
  chatAnswered = null;
  holdAnswer = undefined;
  mockIPC((cmd, args) => {
    const session = (args as { session?: number } | undefined)?.session;
    if (cmd === "task_question")
      return session === 8
        ? { task: "designer 8", asked: "steward 4", number: 2, question: "Which colour?" }
        : { task: "developer 7", asked: "steward 4", number: 1, question: "Which one should win?" };
    if (cmd === "answer_task_question") {
      sent.push(args as Record<string, unknown>);
      // As the core does (#5005): only Say otherwise overrides an answer a chat gave first.
      if (chatAnswered !== null && (args as { overrides?: boolean }).overrides !== true)
        throw "The chat 'steward 4' answered that question of 'developer 7' first.";
      return holdAnswer === undefined ? null : holdAnswer.then(() => null);
    }
    if (cmd === "task_question_answered") return chatAnswered;
    return null;
  });
});
afterEach(() => {
  cleanup();
  clearMocks();
  openTask.mockReset();
  openSettings.mockReset();
});

const quiz = () => screen.queryByRole("group", { name: "developer 7 asks" });
const LINE = "Questions from this chat's tasks now come here, so you answer them in one place.";
/** The Notice whose words are `words`: its box in the band. */
const noticeOf = (words: string | RegExp) =>
  screen.getByText(words).closest('[data-scope="notice"][data-part="root"]') as HTMLElement;

/** Opens the quiz of the row on screen: a question is one row until it is opened (#5215). */
const opened = async () => {
  const toggle = screen.queryByRole("button", { name: "Show the whole question" });
  if (toggle !== null) await userEvent.click(toggle);
};

describe("where a task's question is drawn (#4565)", () => {
  it("draws it from the top by default, in the top's pane and the task's own", () => {
    render(<Pane session={4} asks={[QUESTION]} />);
    expect(quiz()).toBeInTheDocument();
    cleanup();
    render(<Pane session={7} asks={[QUESTION]} />);
    expect(quiz()).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Open task" })).not.toBeInTheDocument();
  });

  it("says once that questions come here, with Change to Settings, and its × keeps the setting for good (#5213)", async () => {
    const user = userEvent.setup();
    render(<Pane session={4} asks={[QUESTION]} />);
    // One way out to the setting and a ×: keeping it is the default and needs no button.
    expect(
      within(noticeOf(LINE))
        .getAllByRole("button")
        .map((button) => button.textContent || button.getAttribute("aria-label")),
    ).toEqual(["Change", "Dismiss"]);
    await user.click(screen.getByRole("button", { name: "Change" }));
    expect(openSettings).toHaveBeenCalled();
    expect(whereYouAnswer()).toEqual({ answer: "top", offered: false });
    await user.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByText(LINE)).not.toBeInTheDocument();
    expect(whereYouAnswer()).toEqual({ answer: "top", offered: true });
    expect(quiz()).toBeInTheDocument();
    cleanup();
    render(<Pane session={4} asks={[QUESTION]} />);
    expect(screen.queryByText(LINE)).not.toBeInTheDocument();
  });

  it("offers nothing where no question has reached the person, nor once it was answered", () => {
    render(<Pane session={4} asks={[]} />);
    expect(screen.queryByText(LINE)).not.toBeInTheDocument();
    cleanup();
    setWhereYouAnswer({ offered: true });
    render(<Pane session={4} asks={[QUESTION]} />);
    expect(screen.queryByText(LINE)).not.toBeInTheDocument();
  });

  it("closes a question in this pane only, answering nothing, and shows the next (#5213)", async () => {
    setWhereYouAnswer({ offered: true });
    const user = userEvent.setup();
    const second: Shown = {
      ...QUESTION,
      session: 8,
      ask: "task-question:8:2",
      chain: ["steward 4", "designer 8"],
      since: (QUESTION.since ?? 0) + 60,
    };
    const { rerender } = render(<Pane session={4} asks={[QUESTION, second]} pauseMs={0} />);
    await user.click(within(quiz() as HTMLElement).getByRole("button", { name: "Close question" }));
    expect(quiz()).not.toBeInTheDocument();
    // Still the registry's: the next list holds it, and this pane leaves it closed.
    rerender(<Pane session={4} asks={[QUESTION, second]} pauseMs={0} />);
    expect(await screen.findByRole("group", { name: "designer 8 asks" })).toBeInTheDocument();
    expect(quiz()).not.toBeInTheDocument();
    // Closing told the core nothing, and left no line saying who answered.
    expect(sent).toEqual([]);
    expect(screen.queryByText(/told developer 7/)).not.toBeInTheDocument();
  });

  it("folds with the pane's notices: minimised, the quiz folds to the band's line and the note into the bell (#5213, #5215)", async () => {
    render(<Pane session={4} asks={[QUESTION]} minimised />);
    await waitFor(() => expect(screen.getByText(LINE)).not.toBeVisible());
    expect(quiz()).toBeNull();
    expect(
      screen.getByRole("button", { name: /^1 waiting · A task's question\s*·\s*Show$/ }),
    ).toBeTruthy();
    cleanup();
    render(<Pane session={4} asks={[QUESTION]} />);
    await waitFor(() => expect(screen.getByText(LINE)).toBeVisible());
    expect(quiz()).toBeVisible();
  });

  it("applies a change of setting to an open question at once, and keeps it in its task's pane", () => {
    setWhereYouAnswer({ offered: true });
    render(<Pane session={4} asks={[QUESTION]} />);
    expect(quiz()).toBeInTheDocument();
    act(() => setWhereYouAnswer({ answer: "each" }));
    expect(quiz()).not.toBeInTheDocument();
    cleanup();
    render(<Pane session={7} asks={[QUESTION]} />);
    expect(quiz()).toBeInTheDocument();
  });

  it("opens the task on Open task, and only on the person's press", async () => {
    setWhereYouAnswer({ offered: true });
    const user = userEvent.setup();
    render(<Pane session={4} asks={[QUESTION]} />);
    await user.click(screen.getByRole("button", { name: "Show the whole question" }));
    expect(openTask).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Open task" }));
    expect(openTask).toHaveBeenCalledWith(7);
  });
});

/** A second question, asked after the first, at the same top. */
const SECOND: Shown = {
  ...QUESTION,
  session: 8,
  ask: "task-question:8:2",
  says: "Which colour?",
  chain: ["steward 4", "designer 8"],
  since: (QUESTION.since ?? 0) + 60,
};

const AGENT_SAID = {
  task: "developer 7",
  from: "steward 4",
  said: "Per project.",
  question: "Which one should win?",
};

describe("several questions at once (#4566)", () => {
  beforeEach(() => setWhereYouAnswer({ offered: true }));

  it("pauses before the next quiz, keeping its place, so a second click lands on nothing", async () => {
    const user = userEvent.setup();
    const { rerender } = render(<Pane session={4} asks={[QUESTION, SECOND]} />);
    await opened();
    const box = screen.getByRole("textbox", { name: "Your answer" });
    await waitFor(() => expect(box).not.toBeDisabled());
    await user.type(box, "Per project{Enter}");
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toMatchObject({ overrides: false });
    // The registry's next list has it gone, and the next one waiting.
    rerender(<Pane session={4} asks={[SECOND]} />);
    expect(screen.getByText("Sent to developer 7.")).toBeInTheDocument();
    expect(screen.getByText("The next question comes in a moment.")).toBeInTheDocument();
    expect(screen.queryByRole("group", { name: "designer 8 asks" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Send" })).not.toBeInTheDocument();
    await waitFor(
      () => expect(screen.getByRole("group", { name: "designer 8 asks" })).toBeInTheDocument(),
      { timeout: PAUSE_MS * 3 },
    );
    expect(screen.queryByText("Sent to developer 7.")).not.toBeInTheDocument();
  });

  it("holds the pause as long as a story asks, for its picture", async () => {
    const user = userEvent.setup();
    const { rerender } = render(
      <Pane session={4} asks={[QUESTION, SECOND]} pauseMs={PAUSE_MS * 10} />,
    );
    await opened();
    const box = screen.getByRole("textbox", { name: "Your answer" });
    await waitFor(() => expect(box).not.toBeDisabled());
    await user.type(box, "Per project{Enter}");
    await waitFor(() => expect(sent).toHaveLength(1));
    rerender(<Pane session={4} asks={[SECOND]} pauseMs={PAUSE_MS * 10} />);
    await act(() => new Promise((done) => setTimeout(done, PAUSE_MS * 2)));
    expect(screen.getByText("Sent to developer 7.")).toBeInTheDocument();
    expect(screen.queryByRole("group", { name: "designer 8 asks" })).not.toBeInTheDocument();
  });

  it("says where the answer went though the list moved on before the core said it was taken", async () => {
    // The registry's next list can reach the window before the answer's own reply does.
    const user = userEvent.setup();
    let taken = () => {};
    holdAnswer = new Promise((done) => (taken = done));
    const { rerender } = render(<Pane session={4} asks={[QUESTION, SECOND]} />);
    await opened();
    const box = screen.getByRole("textbox", { name: "Your answer" });
    await waitFor(() => expect(box).not.toBeDisabled());
    await user.type(box, "Per project{Enter}");
    await waitFor(() => expect(sent).toHaveLength(1));
    rerender(<Pane session={4} asks={[SECOND]} />);
    await act(async () => taken());
    expect(await screen.findByText("Sent to developer 7.")).toBeInTheDocument();
    expect(screen.queryByRole("group", { name: "designer 8 asks" })).not.toBeInTheDocument();
  });

  it("closes quietly when the asking chat answers first, leaving who said what until dismissed", async () => {
    const user = userEvent.setup();
    chatAnswered = AGENT_SAID;
    const { rerender } = render(<Pane session={4} asks={[QUESTION]} />);
    expect(quiz()).toBeInTheDocument();
    rerender(<Pane session={4} asks={[]} />);
    expect(
      await screen.findByText('steward 4 told developer 7: "Per project."'),
    ).toBeInTheDocument();
    expect(quiz()).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    // It stays, whatever the registry lists next (the person's ruling).
    rerender(<Pane session={4} asks={[]} />);
    expect(screen.getByText('steward 4 told developer 7: "Per project."')).toBeInTheDocument();
    // Its ways out in the prototype's order: Go to task, Say otherwise, Dismiss.
    expect(
      within(noticeOf(/told developer 7/))
        .getAllByRole("button")
        .map((button) => button.textContent || button.ariaLabel),
    ).toEqual(["Go to task", "Say otherwise…", "Dismiss"]);
    await user.click(screen.getByRole("button", { name: "Go to task" }));
    expect(openTask).toHaveBeenCalledWith(7);
    await user.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByText(/told developer 7/)).not.toBeInTheDocument();
  });

  it("sends the person's word in place of the chat's on Say otherwise", async () => {
    const user = userEvent.setup();
    chatAnswered = AGENT_SAID;
    const { rerender } = render(<Pane session={4} asks={[QUESTION]} />);
    rerender(<Pane session={4} asks={[]} />);
    await user.click(await screen.findByRole("button", { name: "Say otherwise…" }));
    expect(
      screen.getByText("developer 7 is told this replaces steward 4's answer."),
    ).toBeInTheDocument();
    // The person's own press: the box may take the keyboard, as no quiz arriving ever does.
    const box = screen.getByRole("textbox", { name: "What you want instead" });
    expect(box).toHaveFocus();
    await user.type(box, "Per runner");
    await user.click(screen.getByRole("button", { name: "Send" }));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toMatchObject({
      session: 7,
      number: 1,
      question: "Which one should win?",
      text: "Per runner",
      overrides: true,
    });
    expect(
      await screen.findByText("developer 7 was told your word replaces steward 4's answer."),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByText(/your word replaces/)).not.toBeInTheDocument();
  });

  it("keeps what the person typed when the chat answers first, to send as their word", async () => {
    const user = userEvent.setup();
    chatAnswered = AGENT_SAID;
    const { rerender } = render(<Pane session={4} asks={[QUESTION]} />);
    await opened();
    const box = screen.getByRole("textbox", { name: "Your answer" });
    await waitFor(() => expect(box).not.toBeDisabled());
    await user.type(box, "Per runner");
    rerender(<Pane session={4} asks={[]} />);
    expect(
      await screen.findByText(
        'steward 4 answered first: "Per project." What you typed is still in the box.',
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "What you want instead" })).toHaveValue(
      "Per runner",
    );
    await user.click(screen.getByRole("button", { name: "Send as your word" }));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toMatchObject({ session: 7, number: 1, text: "Per runner", overrides: true });
  });

  it("never turns a Send pressed just after the chat answered into an override", async () => {
    // #5005: the asking chat answered, and the person pressed Enter before the registry dropped
    // the quiz. They never saw the chat's answer, so the core refuses it and the closing line
    // shows that answer, with what they typed still in the box.
    const user = userEvent.setup();
    chatAnswered = AGENT_SAID;
    const { rerender } = render(<Pane session={4} asks={[QUESTION]} />);
    await opened();
    const box = screen.getByRole("textbox", { name: "Your answer" });
    await waitFor(() => expect(box).not.toBeDisabled());
    await user.type(box, "Per runner{Enter}");
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toMatchObject({ text: "Per runner", overrides: false });
    rerender(<Pane session={4} asks={[]} />);
    expect(
      await screen.findByText(
        'steward 4 answered first: "Per project." What you typed is still in the box.',
      ),
    ).toBeInTheDocument();
    expect(screen.queryByText("Sent to developer 7.")).not.toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "What you want instead" })).toHaveValue(
      "Per runner",
    );
  });

  it("leaves no line and no error where the person answered it elsewhere", async () => {
    const { rerender } = render(<Pane session={4} asks={[QUESTION]} />);
    rerender(<Pane session={4} asks={[]} />);
    await waitFor(() => expect(quiz()).not.toBeInTheDocument());
    expect(screen.queryByText(/told developer 7/)).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("offers the choice in one place only, with two tops side by side", () => {
    setWhereYouAnswer({ offered: false });
    const third: Shown = { ...QUESTION, session: 21, ask: "task-question:21:3" };
    render(<Pane session={[4, 20]} asks={[QUESTION, third]} />);
    expect(screen.getAllByText(LINE)).toHaveLength(1);
    expect(screen.getAllByRole("group", { name: "developer 7 asks" })).toHaveLength(2);
  });

  it("links a task's question the window cannot draw to its pane, from the top", async () => {
    const user = userEvent.setup();
    const prompt = {
      session: 7,
      ask: "terminal:7",
      says: "Waiting in its terminal: a question",
      options: [],
      source: "terminal",
      // The registry names every ask's chain from the app's own record.
      chain: ["steward 4", "developer 7"],
      answer: { via: "in-its-pane" },
      since: null,
    } as unknown as Shown;
    render(<Pane session={4} asks={[prompt]} />);
    expect(
      screen.getByText(
        "developer 7 asks something this tab cannot draw. Answer it in the task's pane.",
      ),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Open task" }));
    expect(openTask).toHaveBeenCalledWith(7);

    // It closes as a quiz does: from this pane, the registry keeping it (#5213).
    await user.click(screen.getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(screen.queryByText(/cannot draw/)).toBeNull());
    expect(sent).toEqual([]);
  });
});
