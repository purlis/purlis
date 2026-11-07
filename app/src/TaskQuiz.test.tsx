// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Shown } from "./bindings";
import { TaskQuiz } from "./TaskQuiz";

/**
 * **The quiz a task question is drawn as** (#4565): it says why it reached the person, takes an
 * answer, and never takes the keyboard. The core is a pretend one that answers `task_question`
 * as the real one does while the task is paused on its question, and records what it was sent.
 */

const PLANE = "/home/dev/plane";
const NOW = Date.UTC(2026, 10, 3, 12, 0, 0);

/** A question as `task_question` hands it to the window: the same words, in
 *  `asking::tests::a_task_questions_link_and_raw_html_reach_the_window_as_written`. */
const TASK_PATH_QUESTION =
  'Read [the plan](https://example.com/plan).\n<script>alert(1)</script>\n\nBefore <b onclick="x()">tag</b>.';

function asked(over: Partial<Shown> = {}, answer?: Partial<Shown["answer"]>): Shown {
  return {
    session: 7,
    ask: "task-question:7:3",
    says: "The pool limit is per runner in the docs but per project in the code. Which wins?",
    options: [],
    source: "task-question",
    chain: ["steward 4", "developer 7"],
    answer: {
      via: "task-question",
      why: "passed-on",
      from: "steward 4",
      note: "I can't tell which the docs meant.",
      ...answer,
    } as Shown["answer"],
    since: NOW / 1000 - 120,
    ...over,
  };
}

function core() {
  const sent: unknown[] = [];
  mockIPC((cmd, args) => {
    if (cmd === "task_question")
      return { task: "developer 7", asked: "steward 4", number: 3, question: "Which wins?" };
    if (cmd === "answer_task_question") {
      sent.push(args);
      return null;
    }
    return null;
  });
  return sent;
}

afterEach(() => {
  cleanup();
  clearMocks();
  vi.useRealTimers();
});

function draw(ask: Shown, more = 0, own = false, onOpenTask = vi.fn()) {
  render(
    <>
      <TaskQuiz
        plane={PLANE}
        ask={ask}
        more={more}
        own={own}
        now={NOW}
        onOpenTask={onOpenTask}
        defaultOpen
        onAnswered={() => {}}
      />
    </>,
  );
  return onOpenTask;
}

describe("a task question's quiz (#4565)", () => {
  it("says who asks, why it reached you, the asking chat's note as its own, and how long", () => {
    core();
    draw(asked());
    const quiz = screen.getByRole("group", { name: "developer 7 asks" });
    expect(quiz).toHaveTextContent("developer 7 asks");
    expect(quiz).toHaveTextContent("Which wins?");
    expect(quiz).toHaveTextContent("steward 4 passed this on.");
    expect(quiz).toHaveTextContent("steward 4's note");
    expect(quiz).toHaveTextContent("I can't tell which the docs meant.");
    expect(quiz).toHaveTextContent("2m");
  });

  it("says each reason a question reaches the person", () => {
    core();
    draw(asked({}, { why: "no-reply", note: null }));
    expect(screen.getByText("steward 4 did not reply in 100 seconds.")).toBeInTheDocument();
    expect(screen.queryByText(/'s note/)).not.toBeInTheDocument();
    cleanup();
    draw(asked({}, { why: "asker-gone", note: null }));
    expect(screen.getByText("steward 4 has closed, so this came to you.")).toBeInTheDocument();
  });

  it("shows a question over several lines as its lines, in markdown, never a line break's mark (#5213)", () => {
    core();
    draw(asked({ says: "Which branch?\nSay why.\n\n- **main**\n- or a fork" }));
    const quiz = screen.getByRole("group", { name: "developer 7 asks" });
    expect(quiz.textContent).not.toMatch(/⟦U\+/);
    expect(quiz.querySelector("br")).not.toBeNull();
    expect(within(quiz).getAllByRole("listitem")).toHaveLength(2);
    expect(within(quiz).getByText("main").tagName).toBe("STRONG");
    expect(quiz.textContent).not.toContain("**");
  });

  it("draws a question's link to open only on a press, and drops its raw HTML (#5213)", async () => {
    const urls: string[] = [];
    mockIPC((cmd, args) => {
      if (cmd === "plugin:opener|open_url") urls.push((args as { url: string }).url);
      if (cmd === "task_question")
        return { task: "developer 7", asked: "steward 4", number: 3, question: "Which wins?" };
      return null;
    });
    draw(asked({ says: TASK_PATH_QUESTION }));
    const quiz = screen.getByRole("group", { name: "developer 7 asks" });

    expect(quiz.querySelector("b, script, img")).toBeNull();
    expect(quiz.textContent).not.toContain("alert");
    expect(quiz.textContent).not.toContain("<b");
    expect(within(quiz).getByText("(example.com)")).toBeInTheDocument();
    expect(urls).toEqual([]);

    await userEvent.click(within(quiz).getByRole("link", { name: "the plan" }));

    expect(urls).toEqual(["https://example.com/plan"]);
  });

  it("draws the asking chat's note the same way: its link opens on a press, its raw HTML is dropped (#5213)", async () => {
    const urls: string[] = [];
    mockIPC((cmd, args) => {
      if (cmd === "plugin:opener|open_url") urls.push((args as { url: string }).url);
      if (cmd === "task_question")
        return { task: "developer 7", asked: "steward 4", number: 3, question: "Which wins?" };
      return null;
    });
    draw(asked({}, { note: `**Mine:** ${TASK_PATH_QUESTION}` }));
    const quiz = screen.getByRole("group", { name: "developer 7 asks" });

    expect(within(quiz).getByText("Mine:").tagName).toBe("STRONG");
    expect(quiz.querySelector("b, script, img")).toBeNull();
    expect(quiz.textContent).not.toContain("alert");
    expect(urls).toEqual([]);

    await userEvent.click(within(quiz).getByRole("link", { name: "the plan" }));

    expect(urls).toEqual(["https://example.com/plan"]);
  });

  it("names the question's words as what the answer box answers", () => {
    core();
    draw(asked({ says: "Which **branch**?" }));
    expect(screen.getByRole("textbox", { name: "Your answer" })).toHaveAccessibleDescription(
      expect.stringContaining("Which branch?"),
    );
  });

  it("names the whole chain of a nested task", () => {
    core();
    draw(asked({ chain: ["steward 4", "architect 2", "developer 9", "qa 5"] }));
    expect(
      screen.getByRole("group", { name: "steward 4 › architect 2 › developer 9 › qa 5" }),
    ).toBeInTheDocument();
  });

  it("never takes the keyboard: typing in the terminal carries on", async () => {
    core();
    const user = userEvent.setup();
    render(<textarea aria-label="Terminal" />);
    await user.click(screen.getByLabelText("Terminal"));
    await user.keyboard("pnpm te");
    render(
      <TaskQuiz
        plane={PLANE}
        ask={asked()}
        more={0}
        own={false}
        now={NOW}
        onOpenTask={() => {}}
        defaultOpen
        onAnswered={() => {}}
      />,
    );
    await screen.findByRole("textbox", { name: "Your answer" });
    // The question is read from the core: the box is ready, and still the keyboard stays.
    await waitFor(() =>
      expect(screen.getByRole("textbox", { name: "Your answer" })).not.toBeDisabled(),
    );
    await user.keyboard("st");
    expect(screen.getByLabelText("Terminal")).toHaveFocus();
    expect(screen.getByLabelText("Terminal")).toHaveValue("pnpm test");
  });

  it("sends the person's answer for the question the task is paused on", async () => {
    const sent = core();
    const user = userEvent.setup();
    draw(asked());
    const box = screen.getByRole("textbox", { name: "Your answer" });
    await waitFor(() => expect(box).not.toBeDisabled());
    await user.type(box, "Per project");
    await user.click(screen.getByRole("button", { name: "Send" }));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toMatchObject({ session: 7, number: 3, text: "Per project" });
  });

  it("clears the box and holds Send once the answer is taken, so a second Enter sends nothing", async () => {
    // #4566, from #4938's review: the registry's next list is what takes the quiz away, and
    // until it comes the box held the text and Send was live.
    const sent = core();
    const user = userEvent.setup();
    draw(asked());
    const box = screen.getByRole("textbox", { name: "Your answer" });
    await waitFor(() => expect(box).not.toBeDisabled());
    await user.type(box, "Per project{Enter}");
    await waitFor(() => expect(sent).toHaveLength(1));
    await waitFor(() => expect(box).toHaveValue(""));
    expect(box).toBeDisabled();
    expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
    await user.keyboard("{Enter}");
    expect(sent).toHaveLength(1);
  });

  it("tells what is typed in it to the band, which keeps it should an agent answer first", async () => {
    core();
    const user = userEvent.setup();
    const typed = vi.fn();
    render(
      <TaskQuiz
        plane={PLANE}
        ask={asked()}
        more={0}
        own={false}
        now={NOW}
        onOpenTask={vi.fn()}
        defaultOpen
        onAnswered={() => {}}
        onTyped={typed}
      />,
    );
    const box = screen.getByRole("textbox", { name: "Your answer" });
    await waitFor(() => expect(box).not.toBeDisabled());
    await user.type(box, "Per");
    expect(typed).toHaveBeenLastCalledWith("Per");
  });

  it("counts the questions behind it, and opens its task on Open task", async () => {
    core();
    const user = userEvent.setup();
    const onOpenTask = draw(asked(), 2);
    expect(screen.getByText("2 more")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Open task" }));
    expect(onOpenTask).toHaveBeenCalledWith(7);
  });

  it("has no Open task in the task's own pane", () => {
    core();
    draw(asked(), 0, true);
    expect(screen.queryByRole("button", { name: "Open task" })).not.toBeInTheDocument();
  });
});
