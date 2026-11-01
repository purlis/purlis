// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { GradeRow, SkillDraftGrade, SkillGradeStep } from "./bindings";
import { DraftGradeActions, DraftGradeLine, draftGradeState } from "./SkillDraftGrade";

const ROW: GradeRow = {
  grade: "g1",
  graded: "today 21:40",
  version: "uncommitted",
  ran_on: "claude",
  ended: "Finished",
  outcome: "graded",
  arms: { with: 92, without: 33, previous: 75 },
  spent: { chats: 54, tokens: null, cost_usd: null, limit_usd: null },
  after_edit: false,
};

const NOTHING: SkillDraftGrade = {
  only_evals: false,
  tests: 2,
  evals: 0,
  tests_refused: null,
  latest: null,
  asked: 0,
  changed: false,
  running: null,
};

const STEP = { ran: 21, of: 54, grade: "g2" } as SkillGradeStep;

const graded = (over: Partial<GradeRow>, more: Partial<SkillDraftGrade> = {}): SkillDraftGrade => ({
  ...NOTHING,
  latest: { ...ROW, ...over },
  asked: 54,
  ...more,
});

/** The line and the ways on, as the footer draws them, for `grade` of a change to `deploy`. */
function drawn(grade: SkillDraftGrade, isNew = false) {
  const go = {
    onGrade: vi.fn(),
    onShow: vi.fn(),
    onScorecard: vi.fn(),
    onAskForTests: vi.fn(),
  };
  const state = draftGradeState(grade);
  render(
    <>
      <DraftGradeLine name="deploy" state={state} />
      <DraftGradeActions state={state} isNew={isNew} {...go} />
    </>,
  );
  return go;
}

afterEach(cleanup);

describe("a draft's grade beside Accept", () => {
  it("says it is not graded, and offers the grade against the current version", () => {
    const go = drawn(NOTHING);
    screen.getByText("Not graded yet. A grade shows its cost before it runs.");
    screen.getByRole("button", { name: "Grade against the current version…" }).click();
    expect(go.onGrade).toHaveBeenCalled();
  });

  it("offers a new skill a grade with no current version in its name", () => {
    drawn(NOTHING, true);
    screen.getByRole("button", { name: "Grade…" });
  });

  it("says how far a running grade is, and shows it", () => {
    const go = drawn({ ...NOTHING, running: STEP });
    screen.getByRole("progressbar", { name: "Grading: 21 of 54 chats" });
    screen.getByRole("button", { name: "Show it" }).click();
    expect(go.onShow).toHaveBeenCalledWith("g2");
    expect(screen.queryByRole("button", { name: /Grade/ })).toBeNull();
  });

  it.each([
    ["better", 92, "+17 points"],
    ["worse", 58, "-17 points"],
    ["no different", 75, "No difference"],
  ])("draws a draft that is %s against the current version", (_, draft, difference) => {
    const go = drawn(graded({ arms: { with: draft, without: 33, previous: 75 } }));
    expect(screen.getByTestId("skill-draft-figures").textContent).toContain(
      `Draft ${draft}%Current 75%No skill 33%`,
    );
    screen.getByText(difference);
    screen.getByRole("button", { name: "Scorecard" }).click();
    expect(go.onScorecard).toHaveBeenCalledWith("g1");
    screen.getByRole("button", { name: "Grade again…" });
  });

  it("weighs a new skill's draft against no skill", () => {
    drawn(graded({ arms: { with: 92, without: 33, previous: null } }), true);
    expect(screen.queryByText("Current")).toBeNull();
    screen.getByText("+59 points");
  });

  it("says a grade stopped early compares what ran", () => {
    drawn(
      graded({
        outcome: "stopped",
        arms: { with: 80, without: 33, previous: 70 },
        spent: { ...ROW.spent, chats: 18 },
      }),
    );
    screen.getByText("Stopped after 18 of 54 chats, so this compares what ran.");
    screen.getByText("+10 points");
  });

  it("says a grade that ended with nothing to compare, and offers another", () => {
    drawn(
      graded({
        outcome: "interrupted",
        arms: { with: null, without: null, previous: null },
        spent: { ...ROW.spent, chats: 0 },
      }),
    );
    screen.getByText("Interrupted before anything could be compared.");
    screen.getByRole("button", { name: "Grade again…" });
    expect(screen.queryByText("Not run")).toBeNull();
  });

  it("marks a grade of what the draft held before, with no way to its scorecard", () => {
    drawn(graded({}, { changed: true }));
    screen.getByText("Out of date");
    screen.getByText("The draft changed after this grade.");
    screen.getByText("92%");
    expect(screen.queryByRole("button", { name: "Scorecard" })).toBeNull();
    screen.getByRole("button", { name: "Grade again…" });
  });

  it("offers nothing for a draft that changes only the skill's tests", () => {
    drawn({ ...NOTHING, only_evals: true });
    screen.getByText(
      "This draft changes only the skill's tests, so there is nothing to grade against the current version.",
    );
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("offers to ask for tests where a grade has none to run", () => {
    const go = drawn({ ...NOTHING, tests: 0 });
    screen.getByText("deploy has no trigger tests yet, so a grade has nothing to run.");
    screen.getByRole("button", { name: "Ask a chat to write them" }).click();
    expect(go.onAskForTests).toHaveBeenCalled();
  });

  it("takes outcome evals alone as something to run", () => {
    drawn({ ...NOTHING, tests: 0, evals: 1 });
    screen.getByRole("button", { name: "Grade against the current version…" });
  });

  it("says why tests that could not be read were not, and asks no chat", () => {
    drawn({ ...NOTHING, tests: 0, tests_refused: "evals/triggers.json is not JSON." });
    screen.getByText("evals/triggers.json is not JSON.");
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("asks no chat for a new skill's tests, as it has no folder to write them to yet", () => {
    drawn({ ...NOTHING, tests: 0 }, true);
    expect(screen.queryByRole("button")).toBeNull();
  });
});
