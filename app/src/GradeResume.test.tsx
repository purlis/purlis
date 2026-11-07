// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Resuming a stopped grade** (#5718), drawn at the window seam with the core faked at its
 * command boundary: no grade runs, paid or not.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { GradeScorecardPage } from "./GradeScorecard";
import { leftSaid, resetSaid, resumeFix, resumeLabel } from "./GradeResume";
import type { GradeScorecard, SkillGradeResumePlan } from "./bindings";

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLAN: SkillGradeResumePlan = {
  grade: "01STOPPED",
  skill: "deploy",
  ended: "Stopped at the token cap",
  refused: null,
  waits_until: null,
  left: 14,
  tokens_left: 40000,
  usd_left: 0.3,
  token_cap: 200000,
  cap_set_here: true,
  limit_usd: 1.5,
  earlier_tokens: 240000,
  earlier_usd: 1.71,
  profile: "work",
  model: "sonnet",
  consent: "be5ume",
  running: null,
};

const CARD: GradeScorecard = {
  draft: false,
  grade: "01STOPPED",
  skill: "deploy",
  level: { at: "project", local: false },
  graded: "2026-11-08 10:00 UTC",
  version: "0123456789ab",
  ran_on: "Claude Code · sonnet",
  ended: "Stopped at the token cap",
  delta: null,
  against: null,
  delta_without: null,
  after_edit: false,
  resets_at: null,
  asked: 0,
  outcome: "at-limit",
  arms: { with: 50, without: 0, previous: null },
  outcome_arms: null,
  spent: {
    chats: 10,
    tokens: 240000,
    cost_usd: 1.71,
    limit_usd: 3,
    resumed: false,
    earlier_tokens: null,
    earlier_usd: null,
  },
  evals: [],
  tests: [],
};

describe("Resume, beside Grade again (#5718)", () => {
  it("says what is left to run and the cap in the operator's words", () => {
    expect(leftSaid(PLAN)).toBe("About 40,000 tokens left to run; cap 200,000.");
    expect(leftSaid({ ...PLAN, tokens_left: null })).toBe(
      "14 chats left to run; what they spend is not known yet; cap 200,000.",
    );
  });

  it("says when it can run where the program said, and waits for it", () => {
    const now = new Date("2026-11-08T10:00:00");
    const at = new Date("2026-11-08T14:00:00").toISOString();
    expect(resetSaid(at, now)).toBe("14:00");
    const waiting = { ...PLAN, waits_until: at };
    expect(resumeLabel(waiting, now)).toBe("Resume after your limit resets at 14:00");
    const fix = resumeFix(waiting, () => {}, now);
    expect(fix?.off).toBe("Your usage limit resets at 14:00; Resume can run then.");
    expect(resumeFix(PLAN, () => {}, now)).toMatchObject({ label: "Resume", off: undefined });
  });

  it("is not offered where something that decides the result changed", () => {
    expect(resumeFix({ ...PLAN, refused: "Resume is not offered" }, () => {})).toBeUndefined();
    expect(resumeFix(undefined, () => {})).toBeUndefined();
  });

  it("asks first, then resumes on the plan's consent and opens the grade it started", async () => {
    const resumed = vi.fn();
    const sent: unknown[] = [];
    mockIPC((cmd, args) => {
      if (cmd === "skill_grade_scorecard") return CARD;
      if (cmd === "skill_grade_resume_plan") return PLAN;
      if (cmd === "skill_grade_resume") {
        sent.push(args);
        return "01RESUMED";
      }
      return null;
    });
    render(
      <GradeScorecardPage
        plane="p"
        name="deploy"
        grade="01STOPPED"
        onChat={() => {}}
        onGradeAgain={() => {}}
        onResumed={resumed}
      />,
    );
    fireEvent.click(await screen.findByRole("button", { name: "Resume" }));
    const question = await screen.findByRole("alertdialog", {
      name: "Resume the grade of deploy?",
    });
    expect(question).toHaveTextContent("About 40,000 tokens left to run; cap 200,000.");
    expect(sent).toEqual([]);
    fireEvent.click(within(question).getByRole("button", { name: "Resume" }));
    await waitFor(() => expect(resumed).toHaveBeenCalledWith("01RESUMED"));
    expect(sent).toEqual([expect.objectContaining({ grade: "01STOPPED", consent: "be5ume" })]);
  });

  it("says why it is not offered, and offers Grade again alone", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_grade_scorecard") return CARD;
      if (cmd === "skill_grade_resume_plan")
        return {
          ...PLAN,
          refused:
            "Resume is not offered: its tests changed since it stopped. Grade again runs it whole.",
          consent: null,
        };
      return null;
    });
    render(
      <GradeScorecardPage
        plane="p"
        name="deploy"
        grade="01STOPPED"
        onChat={() => {}}
        onGradeAgain={() => {}}
        onResumed={() => {}}
      />,
    );
    expect(await screen.findByTestId("grade-scorecard-resume-refused")).toHaveTextContent(
      "its tests changed since it stopped",
    );
    expect(screen.getByRole("button", { name: "Grade again" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Resume" })).toBeNull();
  });

  it("asks nothing of a grade that finished", async () => {
    const asked: string[] = [];
    mockIPC((cmd) => {
      asked.push(cmd);
      return cmd === "skill_grade_scorecard" ? { ...CARD, outcome: "graded" } : null;
    });
    render(
      <GradeScorecardPage
        plane="p"
        name="deploy"
        grade="01STOPPED"
        onChat={() => {}}
        onGradeAgain={() => {}}
        onResumed={() => {}}
      />,
    );
    await screen.findByRole("button", { name: "Grade again" });
    expect(asked).not.toContain("skill_grade_resume_plan");
  });
});
