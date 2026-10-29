// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A skill's Grades tab** (#3688, S6), drawn at the window seam with the core faked at its
 * command boundary: the latest grade as bars, every grade with how it ended, the grade-after-
 * each-edit row, a skill with no trigger tests, and where each press goes.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { GradesTab, gradingCrumbTo, gradingPath, type GradingGo } from "./SkillGrades";
import type { GradeRow } from "./bindings";

afterEach(() => {
  cleanup();
  clearMocks();
});

const LEVEL = { at: "project", local: false } as const;

/** A grade's row, `over` said on top of a whole one. */
function row(grade: string, over: Partial<GradeRow> = {}): GradeRow {
  return {
    grade,
    graded: "2026-10-28 10:00 UTC",
    version: "0123456789ab",
    ran_on: "Claude Code · sonnet",
    ended: "Finished",
    outcome: "graded",
    arms: { with: 86, without: 68, previous: 80 },
    spent: { chats: 36, tokens: 300000, cost_usd: 1.92, limit_usd: 3 },
    after_edit: false,
    ...over,
  };
}

const GRADES = [
  row("01C"),
  row("01B", {
    graded: "2026-10-22 09:00 UTC",
    outcome: "at-limit",
    ended: "Stopped at its dollar limit",
    arms: { with: 80, without: 69, previous: null },
  }),
  row("01A", {
    graded: "2026-10-20 08:00 UTC",
    outcome: "interrupted",
    ended: "Interrupted: not scored",
    arms: { with: null, without: null, previous: null },
    spent: { chats: 2, tokens: null, cost_usd: 0.21, limit_usd: null },
  }),
];

/** A plan with `tests` trigger tests, and a grade `running` where one is. */
const plan = (tests = 2, running: string | null = null) => ({
  tests: Array.from({ length: tests }, (_, at) => ({
    query: `ship it ${at}`,
    should_trigger: true,
    hides: false,
  })),
  tests_refused: null,
  running,
});

/** A core answering the tab, and each ask it was sent. */
function core(answers: Record<string, unknown> = {}) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC(
    (cmd, args) => {
      asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
      if (cmd in answers) {
        const answer = answers[cmd];
        if (answer instanceof Error) throw answer.message;
        return typeof answer === "function" ? (answer as () => unknown)() : answer;
      }
      if (cmd === "skill_grades") return { grades: GRADES, said: null };
      if (cmd === "skill_grade_plan") return plan();
      return null;
    },
    { shouldMockEvents: true },
  );
  return asked;
}

/** The Grades tab of deploy, and where each press went. */
function renderTab() {
  const onGrading = vi.fn<GradingGo>();
  render(<GradesTab plane="p" name="deploy" level={LEVEL} changed={0} onGrading={onGrading} />);
  return onGrading;
}

describe("a skill's Grades tab (S6)", () => {
  it("shows the latest grade as bars, and every grade with how it ended", async () => {
    core();
    const onGrading = renderTab();
    const bars = await screen.findByRole("group", { name: "The latest grade of deploy" });
    const meters = within(bars).getAllByRole("meter");
    expect(meters.map((one) => one.getAttribute("aria-valuenow"))).toEqual(["86", "68", "80"]);
    expect(within(bars).getByRole("meter", { name: "Without it" })).toBeTruthy();
    expect(screen.getByTestId("skill-grades-latest")).toHaveTextContent(
      "Latest · 2026-10-28 10:00 UTC · Claude Code · sonnet · $1.92",
    );

    const list = await screen.findByRole("grid", { name: "Grades of deploy" });
    await waitFor(() => expect(within(list).getAllByRole("row")).toHaveLength(4));
    const [, graded, limit, interrupted] = within(list).getAllByRole("row");
    expect(graded).toHaveTextContent("Graded");
    expect(limit).toHaveTextContent("Stopped at its dollar limit");
    expect(limit).toHaveTextContent("—");
    expect(interrupted).toHaveTextContent("Interrupted: not scored");
    expect(interrupted).toHaveTextContent("$0.21");

    fireEvent.click(
      within(limit).getByRole("button", { name: "Scorecard of the grade of 2026-10-22 09:00 UTC" }),
    );
    expect(onGrading).toHaveBeenLastCalledWith({ at: "scorecard", grade: "01B" }, "go");
    expect(within(interrupted).getByRole("button", { name: /Scorecard of/ })).toHaveTextContent(
      "What ran",
    );
  });

  it("explains trigger tests to a skill that has none, and offers no grading after each edit", async () => {
    core({ skill_grades: { grades: [], said: null }, skill_grade_plan: plan(0) });
    renderTab();
    const none = await screen.findByTestId("skill-grades-no-tests");
    expect(none).toHaveTextContent("deploy has no trigger tests yet");
    expect(none).toHaveTextContent("kept with the skill in evals/triggers.json");
    expect(screen.queryByTestId("skill-grades-empty")).toBeNull();
    const turnOn = screen.getByRole("button", { name: "Turn on…" });
    expect(turnOn.getAttribute("aria-disabled") === "true" || turnOn.hasAttribute("disabled")).toBe(
      true,
    );
  });

  it("offers a first grade where it has tests and no grades", async () => {
    core({ skill_grades: { grades: [], said: null } });
    const onGrading = renderTab();
    const empty = await screen.findByTestId("skill-grades-empty");
    fireEvent.click(within(empty).getByRole("button", { name: "Grade deploy…" }));
    expect(onGrading).toHaveBeenCalledWith({ at: "plan" }, "go");
  });

  it("says what grading after each edit may spend before it is on, and turns off in one press", async () => {
    let on: unknown = null;
    const asked = core({ skill_grade_after_edit: () => on, skill_grade_after_edit_off: null });
    const onGrading = renderTab();
    const line = await screen.findByTestId("skill-grade-after-edit");
    expect(line.textContent).toBe(
      "Off. When it's on, each edit you save is graded once, within a limit you accept first.",
    );
    fireEvent.click(screen.getByRole("button", { name: "Turn on…" }));
    expect(onGrading).toHaveBeenCalledWith({ at: "plan", afterEdit: true }, "go");
    expect(asked.some((one) => one.cmd === "skill_grade_after_edit_on")).toBe(false);
    cleanup();

    on = {
      profile: "work",
      model: "sonnet",
      chats: 12,
      token_cap: 200000,
      limit_usd: 15,
      waits: null,
    };
    renderTab();
    await waitFor(() =>
      expect(screen.getByTestId("skill-grade-after-edit").textContent).toBe(
        "Each edit you save grades this skill on work, sonnet: up to 12 chats, and at most $15.00 or 200,000 tokens, each time, paid by that profile's account.",
      ),
    );
    on = null;
    fireEvent.click(screen.getByRole("button", { name: "Turn off" }));
    await waitFor(() =>
      expect(asked.some((one) => one.cmd === "skill_grade_after_edit_off")).toBe(true),
    );
    await waitFor(() =>
      expect(screen.getByTestId("skill-grade-after-edit").textContent).toMatch(/^Off\./),
    );
  });

  it("says why an edit was not graded, and that it waits for you", async () => {
    let waits: string | null = null;
    core({
      skill_grade_after_edit: () => ({
        profile: "work",
        model: null,
        chats: 12,
        token_cap: 200000,
        limit_usd: 15,
        waits,
      }),
    });
    renderTab();
    await screen.findByText(/^Each edit you save grades this skill on work/);
    waits = "A grade of it now runs 18 chats, more than the 12 you accepted.";
    await emit("skill-grade-after-edit", { plane: "p", skill: "deploy", said: waits });
    await screen.findByText(waits);
    await waitFor(() =>
      expect(screen.getByTestId("skill-grade-after-edit").textContent).toBe(
        `It waits for you: ${waits}`,
      ),
    );
  });

  it("shows a grade running, one press away", async () => {
    core({ skill_grade_plan: plan(2, "01RUN") });
    const onGrading = renderTab();
    await screen.findByTestId("skill-grades-running");
    fireEvent.click(screen.getByRole("button", { name: "Show it" }));
    expect(onGrading).toHaveBeenCalledWith({ at: "running", grade: "01RUN" }, "go");
  });

  it("says in place why its grades could not be read, with Try again", async () => {
    let reads = 0;
    core({
      skill_grades: () => {
        reads += 1;
        if (reads === 1) throw "The grade record could not be read.";
        return { grades: GRADES, said: null };
      },
    });
    renderTab();
    const failed = await screen.findByTestId("skill-grades-not-read");
    expect(failed).toHaveTextContent("The grade record could not be read.");
    // The rest of the tab still works.
    expect(screen.getByTestId("skill-grades-after-edit")).toBeTruthy();
    fireEvent.click(within(failed).getByRole("button", { name: "Try again" }));
    await screen.findByRole("group", { name: "The latest grade of deploy" });
  });
});

describe("the grading pages' path (S6)", () => {
  it("names each page after the skill's Grades, and a chat's way back to its scorecard", () => {
    expect(gradingPath({ at: "plan" }).map((one) => one.label)).toEqual(["New grade"]);
    expect(gradingPath({ at: "plan", afterEdit: true }).map((one) => one.label)).toEqual([
      "Grade after each edit",
    ]);
    expect(gradingPath({ at: "running", grade: "g" }).map((one) => one.label)).toEqual([
      "Grading…",
    ]);
    const chat = { at: "chat", grade: "g", chat: "c" } as const;
    expect(gradingPath(chat).map((one) => one.label)).toEqual(["Scorecard", "Saved chat"]);
    expect(gradingCrumbTo(chat, "scorecard")).toEqual({ at: "scorecard", grade: "g" });
    expect(gradingCrumbTo(chat, "grading")).toBeUndefined();
  });
});
