// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A grade's scorecard and its saved chats, as pages** (SK-59, #3688 S6), drawn at the window
 * seam with the core faked at its command boundary.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  GradeChatPage,
  GradeScorecardPage,
  differenceSaid,
  evalSaid,
  keptEvalChats,
  runsSaid,
} from "./GradeScorecard";
import { studioOfSkillTab, skillGrading, AT_THE_ROOT } from "./skillStudio";
import type { GradeScorecard } from "./bindings";

afterEach(() => {
  cleanup();
  clearMocks();
});

// jsdom has no `matchMedia`, and xterm asks it for the display's pixel ratio as it opens.
vi.stubGlobal("matchMedia", (query: string) => ({
  matches: false,
  media: query,
  addEventListener: () => {},
  removeEventListener: () => {},
  addListener: () => {},
  removeListener: () => {},
}));

const CARD: GradeScorecard = {
  grade: "01GRADE",
  skill: "deploy",
  level: { at: "project", local: false },
  graded: "2026-10-28 10:00 UTC",
  version: "0123456789ab",
  ran_on: "Claude Code · claude-sonnet-4-5",
  ended: "Finished",
  delta: 33,
  against: null,
  delta_without: null,
  after_edit: false,
  asked: 1,
  outcome: "graded",
  arms: { with: 100, without: 0, previous: null },
  outcome_arms: null,
  spent: { chats: 5, tokens: 40000, cost_usd: 1.92, limit_usd: 3 },
  evals: [],
  tests: [
    {
      query: "ship it to production",
      should_trigger: true,
      arms: [
        {
          arm: "with",
          runs: 3,
          triggered: 2,
          pass: true,
          asked: 0,
          chats: [
            { id: "01A", run: 1, kept: true },
            { id: "01B", run: 2, kept: false },
            { id: "01C", run: 3, kept: true },
          ],
        },
        {
          arm: "without",
          runs: 2,
          triggered: 0,
          pass: false,
          asked: 1,
          chats: [
            { id: "01D", run: 1, kept: true },
            { id: "01E", run: 2, kept: true },
            { id: "01F", run: 3, kept: true },
          ],
        },
      ],
    },
  ],
};

/** The scorecard page of deploy's grade 01GRADE, and what it told the studio. */
function renderCard() {
  const onChat = vi.fn<(chat: string) => void>();
  const onGradeAgain = vi.fn();
  render(
    <GradeScorecardPage
      plane="p"
      name="deploy"
      grade="01GRADE"
      onChat={onChat}
      onGradeAgain={onGradeAgain}
    />,
  );
  return { onChat, onGradeAgain };
}

describe("A grade's scorecard page (SK-59, S6)", () => {
  it("comes back as a page in the studio from a scorecard or chat tab an older launch kept", () => {
    expect(studioOfSkillTab({ from: null, view: "skill-grade", key: "deploy@01G" })).toEqual(
      skillGrading(AT_THE_ROOT, "deploy", { at: "scorecard", grade: "01G" }),
    );
    expect(
      studioOfSkillTab({ from: null, view: "skill-grade-chat", key: "deploy@01G@01C" }),
    ).toEqual(skillGrading(AT_THE_ROOT, "deploy", { at: "chat", grade: "01G", chat: "01C" }));
    expect(studioOfSkillTab({ from: null, view: "skill-grade", key: "deploy" })).toBeUndefined();
    expect(
      studioOfSkillTab({ from: null, view: "skill-grade", key: "deploy@01G@01C" }),
    ).toBeUndefined();
    expect(
      studioOfSkillTab({ from: "ext", view: "skill-grade-chat", key: "a@b@c" }),
    ).toBeUndefined();
  });

  it("says each arm's runs, its verdict and the chats that asked you something", () => {
    expect(runsSaid(CARD.tests[0].arms[0], true)).toBe(
      "2 of 3 chats started it (should start it), passed",
    );
    expect(runsSaid(CARD.tests[0].arms[1], true)).toBe(
      "0 of 2 chats started it (should start it), failed; 1 chat asked you something, and purlis ended it there without counting it",
    );
  });

  it("opens with how it ended, then its totals, then a row per test with each kept chat", async () => {
    mockIPC((cmd) => (cmd === "skill_grade_scorecard" ? CARD : null));
    const told = renderCard();
    const about = await screen.findByTestId("grade-scorecard-about");
    expect(about.textContent).toBe(
      "Graded 2026-10-28 10:00 UTC, version 0123456789ab, on Claude Code · claude-sonnet-4-5 for $1.92. Finished, the skill made a difference of +33 points.",
    );
    const totals = screen.getByRole("group", { name: "Totals of the grade of deploy" });
    expect(totals).toHaveTextContent("With it100%of tests passed");
    expect(totals).toHaveTextContent("Without it0%+100 points");
    expect(totals).toHaveTextContent("Spent$1.92of $3.00 · 5 chats");
    expect(screen.getByTestId("grade-scorecard-asked").textContent).toContain(
      "1 chat asked you something",
    );
    const with_ = await screen.findByTestId("grade-scorecard-with");
    expect(with_).toHaveTextContent("2 of 3");
    expect(with_).toHaveTextContent("Passed");
    const buttons = with_.querySelectorAll("button");
    expect(buttons).toHaveLength(3);
    // A disabled button with a hint keeps its focus: it is marked, not removed.
    expect(buttons[1].getAttribute("aria-disabled")).toBe("true");
    fireEvent.click(buttons[2]);
    expect(told.onChat).toHaveBeenCalledWith("01C");
    fireEvent.click(screen.getByRole("button", { name: "Grade again" }));
    expect(told.onGradeAgain).toHaveBeenCalledTimes(1);
  });

  it("says a grade that stopped at its limit, or was interrupted, in the tone of its end", async () => {
    mockIPC((cmd) =>
      cmd === "skill_grade_scorecard"
        ? { ...CARD, outcome: "at-limit", ended: "Stopped at its dollar limit" }
        : null,
    );
    renderCard();
    const about = await screen.findByTestId("grade-scorecard-about");
    expect(about.textContent).toContain("Stopped at its limit 2026-10-28");
    expect(
      about.closest('[data-scope="notice"][data-part="root"]')?.getAttribute("data-variant"),
    ).toBe("attention");
    cleanup();
    clearMocks();

    mockIPC((cmd) =>
      cmd === "skill_grade_scorecard"
        ? { ...CARD, outcome: "interrupted", ended: "Interrupted: not scored" }
        : null,
    );
    renderCard();
    expect((await screen.findByTestId("grade-scorecard-about")).textContent).toContain(
      "Interrupted 2026-10-28",
    );
    // Nothing was compared, so there are no totals to show.
    expect(screen.queryByRole("group", { name: "Totals of the grade of deploy" })).toBeNull();
  });

  it("says the difference against the previous version, and against no skill (SK-57)", () => {
    expect(differenceSaid(CARD)).toBe(", the skill made a difference of +33 points.");
    expect(differenceSaid({ ...CARD, against: "fedcba987654", delta: 25, delta_without: 75 })).toBe(
      ", this version made a difference of +25 points against its previous version fedcba987654, and the skill +75 against no skill.",
    );
    expect(differenceSaid({ ...CARD, delta: null })).toBe(".");
  });

  it("shows the previous version's arm, and says a grade a saved edit started (SK-57, SK-58)", async () => {
    const previous = {
      ...CARD.tests[0].arms[0],
      arm: "previous" as const,
      chats: [],
    };
    mockIPC((cmd) =>
      cmd === "skill_grade_scorecard"
        ? {
            ...CARD,
            against: "fedcba987654",
            after_edit: true,
            tests: [{ ...CARD.tests[0], arms: [...CARD.tests[0].arms, previous] }],
          }
        : null,
    );
    renderCard();
    const about = await screen.findByTestId("grade-scorecard-about");
    expect(about.textContent).toContain("after an edit you saved");
    expect(about.textContent).toContain("against its previous version fedcba987654");
    expect(await screen.findByRole("columnheader", { name: "Its previous version" })).toBeTruthy();
    expect((await screen.findByTestId("grade-scorecard-previous")).textContent).toContain("2 of 3");
  });

  it("shows each outcome eval, its chats and judges, what they found, and why a run failed (SK-55)", async () => {
    const card: GradeScorecard = {
      ...CARD,
      tests: [],
      arms: { with: null, without: null, previous: null },
      outcome_arms: { with: 100, without: 0, previous: null },
      evals: [
        {
          id: "4",
          expectations: ["It is filled"],
          arms: [
            {
              arm: "with",
              runs: 1,
              points: 100,
              results: [
                {
                  run: 1,
                  chat: { id: "01E", run: 1, kept: true },
                  judge: { id: "01J", run: 1, kept: false },
                  passed: 1,
                  checks: [{ passed: true, evidence: "form.txt names Ada" }],
                  failed: null,
                },
              ],
            },
            {
              arm: "without",
              runs: 1,
              points: 0,
              results: [
                {
                  run: 1,
                  chat: { id: "01F", run: 1, kept: true },
                  judge: null,
                  passed: 0,
                  checks: [],
                  failed: "Its chat asked for the tool Bash.",
                },
              ],
            },
          ],
        },
      ],
    };
    mockIPC((cmd) => (cmd === "skill_grade_scorecard" ? card : null));
    const told = renderCard();
    const totals = await screen.findByRole("group", {
      name: "Outcome evals of the grade of deploy",
    });
    expect(totals).toHaveTextContent("With it100%of expectations passed");
    expect(totals).toHaveTextContent("Without it0%+100 points");
    expect(screen.queryByTestId("grade-scorecard-tests")).toBeNull();
    expect(screen.queryByRole("group", { name: "Totals of the grade of deploy" })).toBeNull();
    expect(screen.getByTestId("grade-scorecard-asked")).toHaveTextContent(
      "an outcome eval's fails that run",
    );
    expect(screen.getByTestId("grade-scorecard-eval-why")).toHaveTextContent(
      "Eval 4, without it, run 1: Its chat asked for the tool Bash.",
    );
    const expectation = screen.getByTestId("grade-scorecard-expectation");
    expect(expectation).toHaveTextContent("It is filled");
    expect(expectation).toHaveTextContent("With the skill: 1 of 1 passed");
    expect(expectation).toHaveTextContent("Without it: 0 of 1 passed");
    expect(expectation).toHaveTextContent("With the skill, run 1: form.txt names Ada");
    fireEvent.click(
      screen.getByRole("button", { name: "Open with the skill, chat 1, of outcome eval 4" }),
    );
    expect(told.onChat).toHaveBeenCalledWith("01E");
    const judge = screen.getByRole("button", {
      name: "Open with the skill, judge 1, of outcome eval 4",
    });
    expect(judge.hasAttribute("disabled") || judge.getAttribute("aria-disabled") === "true").toBe(
      true,
    );
  });

  it("walks an outcome eval's kept chats, and says a row too long kept no verdicts (SK-55)", () => {
    const run = {
      run: 1,
      chat: { id: "01E", run: 1, kept: true },
      judge: { id: "01J", run: 1, kept: true },
      passed: 1,
      checks: [],
      failed: null,
    };
    const eval4 = {
      id: "4",
      expectations: ["It is filled"],
      arms: [{ arm: "with" as const, runs: 1, points: 100, results: [run] }],
    };
    expect(keptEvalChats([eval4]).map((one) => [one.chat.id, one.about])).toEqual([
      ["01E", "outcome eval 4"],
      ["01J", "the judge of outcome eval 4"],
    ]);
    expect(evalSaid(eval4.arms[0])).toBe("100% over 1 run");
    expect(evalSaid(undefined)).toBe("Not judged");
    expect(
      evalSaid({
        ...eval4.arms[0],
        points: 0,
        results: [{ ...run, passed: 0, failed: "It asked." }],
      }),
    ).toBe("0% over 1 run. It asked.");
  });

  it("says why a grade it cannot read is not shown", async () => {
    mockIPC(() => {
      throw "That grade of deploy is not in the record any more.";
    });
    render(
      <GradeScorecardPage
        plane="p"
        name="deploy"
        grade="01GONE"
        onChat={() => undefined}
        onGradeAgain={() => undefined}
      />,
    );
    expect((await screen.findByTestId("grade-scorecard-refused")).textContent).toContain(
      "not in the record any more",
    );
  });
});

describe("A grade's chat, read-only (SK-59, S6)", () => {
  /** The page of chat `chat` of deploy's grade 01GRADE. */
  function renderChat(chat: string) {
    const onChat = vi.fn<(chat: string) => void>();
    const onScorecard = vi.fn();
    render(
      <GradeChatPage
        plane="p"
        name="deploy"
        grade="01GRADE"
        chat={chat}
        onChat={onChat}
        onScorecard={onScorecard}
      />,
    );
    return { onChat, onScorecard };
  }

  it("draws what the chat showed, takes no input, and walks the grade's kept chats", async () => {
    const asked: Array<{ cmd: string; args: unknown }> = [];
    mockIPC((cmd, args) => {
      asked.push({ cmd, args });
      if (cmd === "skill_grade_scorecard") return CARD;
      return cmd === "skill_grade_chat"
        ? "> ship it to production\r\nUsing the deploy skill"
        : null;
    });
    const told = renderChat("01C");
    await screen.findByTestId("grade-chat");
    expect(asked.find((one) => one.cmd === "skill_grade_chat")).toMatchObject({
      args: { plane: "p", skill: "deploy", grade: "01GRADE", chat: "01C" },
    });
    expect(await screen.findByText(/It takes no input/)).toBeTruthy();
    await waitFor(() =>
      expect(
        screen.getByRole("heading", { name: "Chat 3 of the 2026-10-28 10:00 UTC grade" }),
      ).toBeTruthy(),
    );
    expect(screen.getByText(/With the skill · trigger test “ship it to production”/)).toBeTruthy();
    // The kept chats in order: 01A, 01C (01B is not kept), then the without arm's 01D.
    fireEvent.click(screen.getByRole("button", { name: "Previous chat" }));
    expect(told.onChat).toHaveBeenLastCalledWith("01A");
    fireEvent.click(screen.getByRole("button", { name: "Next chat" }));
    expect(told.onChat).toHaveBeenLastCalledWith("01D");
    fireEvent.click(screen.getByRole("button", { name: "Back to the scorecard" }));
    expect(told.onScorecard).toHaveBeenCalledTimes(1);
  });

  it("says a chat this machine did not keep is not kept here", async () => {
    mockIPC(() => {
      throw "purlis keeps a grade's chats on the machine that ran it.";
    });
    renderChat("01B");
    expect((await screen.findByTestId("grade-chat-refused")).textContent).toContain(
      "on the machine that ran it",
    );
  });
});
