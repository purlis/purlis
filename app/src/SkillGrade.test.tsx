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
    token_cap: 200000,
    cap_set_here: false,
    chat_minutes: 5,
    profile: "work",
    sandbox: "full sandbox",
    refused: null,
    consent: "c0ffee",
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
    expect(cost.textContent).toContain("up to 12 chats × 5 min");
    expect(cost.textContent).toContain(
      "in the project's full sandbox, with none of your own Allows",
    );
    expect(cost.textContent).toContain(
      "2 trigger tests 3 times with the skill and 3 times without it",
    );
    expect(cost.textContent).toContain("has not been graded here before");
    expect(cost.textContent).toContain("its account pays for them");
    expect(cost.textContent).toContain(
      "It stops once its chats have spent 200,000 tokens, the default cap.",
    );
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
      consent: "c0ffee",
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
      tokens: 8000,
      cap: 200000,
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
        tokens: null,
        cap: 200000,
        ended: "chats-did-not-start",
        said: "The default profile is signed out.",
      }),
    ).toBe("The grade stopped: its chats did not start. The default profile is signed out.");
  });
});

describe("a grade's token cap", () => {
  it("is set from the plan, before anything runs, and the plan says it again", async () => {
    const asked: Array<{ cmd: string; args: unknown }> = [];
    mockIPC((cmd, args) => {
      asked.push({ cmd, args });
      if (cmd === "skill_grade_plan") return plan();
      if (cmd === "skill_grade_set_cap") return plan({ token_cap: 50000, cap_set_here: true });
      if (cmd === "start_options") return { profiles: [] };
      return null;
    });
    render(<SkillGrade plane="p" name="deploy" level={LEVEL} onGraded={() => undefined} />);
    fireEvent.click(screen.getByRole("button", { name: "Grade…" }));
    const field = await screen.findByRole("textbox", { name: "Token cap" });
    fireEvent.change(field, { target: { value: "50,000" } });
    fireEvent.click(screen.getByRole("button", { name: "Set the cap" }));
    await waitFor(() =>
      expect(screen.getByTestId("skill-grade-cost").textContent).toContain(
        "have spent 50,000 tokens, the cap you set on this machine.",
      ),
    );
    expect(asked.find((one) => one.cmd === "skill_grade_set_cap")?.args).toMatchObject({
      tokens: 50000,
    });
    expect(asked.some((one) => one.cmd === "skill_grade_start")).toBe(false);
  });

  it("says a grade stopped at its cap, and what it spent", () => {
    expect(
      endedSaid({
        plane: "p",
        skill: "deploy",
        grade: "g",
        ran: 9,
        of: 12,
        tokens: 201345,
        cap: 200000,
        ended: "over-cap",
        said: null,
      }),
    ).toBe(
      "Stopped at its cap of 200,000 tokens after 9 chats, having spent 201,345 tokens. What ran is scored in Grades below.",
    );
  });
});

describe("a grade that cannot run, or runs already", () => {
  it("says why no grade can run on the profile, and offers no Grade press", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_grade_plan")
        return plan({
          sandbox: null,
          refused:
            "Its chats would start with the sandbox off, and a grade runs only in the full sandbox.",
        });
      if (cmd === "start_options") return { profiles: [] };
      return null;
    });
    render(<SkillGrade plane="p" name="deploy" level={LEVEL} onGraded={() => undefined} />);
    fireEvent.click(screen.getByRole("button", { name: "Grade…" }));
    const said = await screen.findByTestId("skill-grade-cannot-run");
    expect(said.textContent).toContain(
      "No grade can run on work: Its chats would start with the sandbox off",
    );
    expect(
      screen.getByRole("button", { name: "Grade" }).hasAttribute("disabled") ||
        screen.getByRole("button", { name: "Grade" }).getAttribute("aria-disabled") === "true",
    ).toBe(true);
  });

  it("shows a grade running from before the tab was drawn, with its Stop", async () => {
    const asked: Array<{ cmd: string; args: unknown }> = [];
    mockIPC((cmd, args) => {
      asked.push({ cmd, args });
      if (cmd === "skill_grade_plan") return plan({ running: "01RUNNING" });
      return null;
    });
    render(<SkillGrade plane="p" name="deploy" level={LEVEL} onGraded={() => undefined} />);
    await screen.findByTestId("skill-grade-running");
    fireEvent.click(screen.getByRole("button", { name: "Stop" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_grade_stop")?.args).toMatchObject({
        grade: "01RUNNING",
      }),
    );
  });
});
