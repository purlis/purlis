// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Shown } from "./bindings";
import { QuestionsHere } from "./QuestionsHere";
import { QuizBand, QuizPlaces } from "./QuizBand";
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

function Pane({ session, asks }: { session: number; asks: Shown[] }) {
  return (
    <QuizPlaces.Provider
      value={{ topOf: (task) => (task === 7 ? 4 : undefined), openTask, openSettings }}
    >
      <QuestionsHere.Provider value={{ asks, with_asking_chat: [] }}>
        <QuizBand plane={PLANE} session={session} />
      </QuestionsHere.Provider>
    </QuizPlaces.Provider>
  );
}

beforeEach(() => {
  forgetThisLaunch();
  mockIPC((cmd) =>
    cmd === "task_question"
      ? { task: "developer 7", asked: "steward 4", number: 1, question: "Which one should win?" }
      : null,
  );
});
afterEach(() => {
  cleanup();
  clearMocks();
  openTask.mockReset();
});

const quiz = () => screen.queryByRole("group", { name: "developer 7 asks" });
const LINE = "Questions from tasks come to this tab, so you answer them in one place.";

describe("where a task's question is drawn (#4565)", () => {
  it("draws it from the top by default, in the top's pane and the task's own", () => {
    render(<Pane session={4} asks={[QUESTION]} />);
    expect(quiz()).toBeInTheDocument();
    cleanup();
    render(<Pane session={7} asks={[QUESTION]} />);
    expect(quiz()).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Open task" })).not.toBeInTheDocument();
  });

  it("offers the choice once, as a quiet line above the first question, and Keep this keeps it", async () => {
    const user = userEvent.setup();
    render(<Pane session={4} asks={[QUESTION]} />);
    expect(screen.getByText(LINE)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Keep this" }));
    expect(screen.queryByText(LINE)).not.toBeInTheDocument();
    expect(whereYouAnswer()).toEqual({ answer: "top", offered: true });
    expect(quiz()).toBeInTheDocument();
  });

  it("offers nothing where no question has reached the person, nor once it was answered", () => {
    render(<Pane session={4} asks={[]} />);
    expect(screen.queryByText(LINE)).not.toBeInTheDocument();
    cleanup();
    setWhereYouAnswer({ offered: true });
    render(<Pane session={4} asks={[QUESTION]} />);
    expect(screen.queryByText(LINE)).not.toBeInTheDocument();
  });

  it("moves the question to its task's own tab on Answer from each session, and says how to go back", async () => {
    const user = userEvent.setup();
    render(<Pane session={4} asks={[QUESTION]} />);
    await user.click(screen.getByRole("button", { name: "Answer from each session" }));
    expect(whereYouAnswer()).toEqual({ answer: "each", offered: true });
    expect(quiz()).not.toBeInTheDocument();
    expect(screen.getByText(/Questions now wait in each task's tab/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Open settings" }));
    expect(openSettings).toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByText(/Questions now wait/)).not.toBeInTheDocument();
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
    expect(openTask).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Open task" }));
    expect(openTask).toHaveBeenCalledWith(7);
  });
});
