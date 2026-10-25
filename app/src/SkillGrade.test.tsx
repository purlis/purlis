// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { SkillGrade, costSaid, endedSaid } from "./SkillGrade";
import type { SkillGradePlan } from "./bindings";

afterEach(() => {
  cleanup();
  clearMocks();
});

function plan(over: Partial<SkillGradePlan> = {}): SkillGradePlan {
  return {
    tests: [
      { query: "ship it to production", should_trigger: true, hides: false },
      { query: "what is prod⟦U+202E⟧", should_trigger: false, hides: true },
    ],
    chats: 12,
    runs: 3,
    tests_refused: null,
    left_out: [],
    outcome_evals: 1,
    outcomes_refused: null,
    estimate: null,
    running: null,
    ...over,
  };
}

const LEVEL = { at: "project", local: false } as const;

describe("Grade, cost first", () => {
  it("shows what a grade runs and who pays before anything runs, then starts it on the press", async () => {
    const asked: Array<{ cmd: string; args: unknown }> = [];
    mockIPC(
      (cmd, args) => {
        asked.push({ cmd, args });
        if (cmd === "skill_grade_plan") return plan();
        if (cmd === "start_options") return { profiles: [{ name: "work" }] };
        if (cmd === "skill_grade_start") return "01GRADE";
        if (cmd === "skill_grade_stop") return null;
        return null;
      },
      { shouldMockEvents: true },
    );
    let graded = 0;
    render(<SkillGrade plane="p" name="deploy" level={LEVEL} onGraded={() => (graded += 1)} />);
    fireEvent.click(screen.getByRole("button", { name: "Grade…" }));
    const cost = await screen.findByTestId("skill-grade-cost");
    expect(cost.textContent).toContain("12 chats, one turn each");
    expect(cost.textContent).toContain(
      "2 trigger tests 3 times with the skill and 3 times without it",
    );
    expect(cost.textContent).toContain("has not been graded here before");
    expect(cost.textContent).toContain("its account pays for them");
    expect(screen.getAllByTestId("skill-grade-test")[1].textContent).toContain(
      "holds a character no reader would see",
    );
    expect(screen.getByText(/1 outcome eval waits/)).toBeTruthy();
    expect(asked.some((one) => one.cmd === "skill_grade_start")).toBe(false);

    fireEvent.click(screen.getByRole("button", { name: "Grade" }));
    await screen.findByTestId("skill-grade-running");
    expect(asked.find((one) => one.cmd === "skill_grade_start")?.args).toMatchObject({
      plane: "p",
      skill: "deploy",
      profile: null,
    });

    fireEvent.click(screen.getByRole("button", { name: "Stop" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_grade_stop")?.args).toMatchObject({
        grade: "01GRADE",
      }),
    );

    await emit("skill-grade", {
      plane: "p",
      skill: "deploy",
      grade: "01GRADE",
      ran: 4,
      of: 12,
      ended: "stopped",
      said: null,
    });
    await screen.findByText("Stopped after 4 chats. What ran is scored in Grades below.");
    expect(graded).toBe(1);
  });

  it("says a skill with no trigger tests has none, and starts nothing", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_grade_plan") return plan({ tests: [], chats: 0, outcome_evals: 0 });
      if (cmd === "start_options") return { profiles: [] };
      return null;
    });
    render(<SkillGrade plane="p" name="deploy" level={LEVEL} onGraded={() => undefined} />);
    fireEvent.click(screen.getByRole("button", { name: "Grade…" }));
    await screen.findByTestId("skill-grade-none");
    expect(screen.queryByRole("button", { name: "Grade" })).toBeNull();
  });

  it("says what the last grade spent per chat, and never a figure it did not measure", () => {
    expect(costSaid(plan({ estimate: { tokens: 120000, cost_usd: 1.2, chats_then: 6 } }))).toBe(
      "From its last grade, of 6 chats, this one should spend about 120,000 tokens, about $1.20.",
    );
    expect(costSaid(plan({ estimate: { tokens: null, cost_usd: null, chats_then: 6 } }))).toContain(
      "cannot say yet",
    );
    expect(
      endedSaid({
        plane: "p",
        skill: "deploy",
        grade: "g",
        ran: 1,
        of: 12,
        ended: "chats-did-not-start",
        said: "The default profile is signed out.",
      }),
    ).toBe("The grade stopped: its chats did not start. The default profile is signed out.");
  });
});
