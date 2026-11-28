// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Grade, in the Library's Select mode** (#5356), at the window seam: the estimate before
 * anything runs, from each skill's last grade, saying where it has none to go on; the skills with
 * no tests left out and named; the limit the person sets; and the grades running, a few at a
 * time, until the limit stops them. The core is faked at its command boundary: what a batch
 * starts and stops is the core's seam test (`grades::batch`), and no grade runs here.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type {
  BatchSkill,
  GradeEstimate,
  SkillGradesPlan,
  SkillGradesStep,
  StudioSkill,
} from "./bindings";
import { estimateSaid, resumeEstimateSaid, stoppedShortOf } from "./SkillLibraryGrade";
import { forgetStudio, PLANE, renderStudio } from "./SkillStudioTab.testkit";
import { AT_THE_ROOT } from "./skillStudio";
import { skill } from "./skillsListing.testkit";

afterEach(() => {
  cleanup();
  clearMocks();
  forgetStudio();
});

const PROJECT = { at: "project", local: false } as const;

/** Whether `button` can't be pressed now: disabled, or unavailable with its reason. */
const unavailable = (button: HTMLElement) =>
  button.hasAttribute("disabled") || button.getAttribute("aria-disabled") === "true";

function lastGrade(cost_usd: number | null): GradeEstimate {
  return {
    tokens: cost_usd === null ? null : 1000,
    cost_usd,
    chats_then: 6,
    ts: "2026-11-01T10:00:00Z",
    profile: "work",
    harness: "claude",
    model: null,
    same_profile: true,
    interrupted: false,
  };
}

function graded(name: string, estimate: GradeEstimate | null): BatchSkill {
  return {
    level: PROJECT,
    name,
    chats: 6,
    limit_usd: 3,
    estimate,
    consent: `c-${name}`,
    resume: null,
  };
}

const PLAN: SkillGradesPlan = {
  skills: [
    graded("deploy", lastGrade(0.4)),
    graded("lint", null),
    graded("review", lastGrade(null)),
  ],
  left_out: [{ level: PROJECT, name: "tidy", why: "It has no tests." }],
  known_usd: 0.4,
  known: 1,
  unknown: ["lint", "review"],
  most_usd: 9,
  at_once: 3,
  profile: "work",
  refused: null,
  running: null,
};

/** A core that lists `skills`, plans `plan`, and says `running` of the batch once one starts. */
function core(
  skills: StudioSkill[],
  plan: SkillGradesPlan,
  running: SkillGradesStep | null = null,
) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  let started = false;
  mockIPC(
    (cmd, args) => {
      const given = (args ?? {}) as Record<string, unknown>;
      asked.push({ cmd, args: given });
      switch (cmd) {
        case "skill_studio":
          return {
            on: true,
            workspace: null,
            persona: null,
            first: { starts: false, unmoved: null },
            places: [],
            workspaces: [],
            levels: [{ level: PROJECT, label: "project" }],
            harnesses: [],
            clashes: [],
            skills,
          };
        case "skills_deleted":
        case "skill_drafts":
          return [];
        case "skill_deletes_cut_short":
          return { settled: [], unsettled: [] };
        case "skill_grades_plan":
          return plan;
        case "skill_grades_start":
          started = true;
          return "b1";
        case "skill_grades_running":
          return started ? running : null;
        default:
          return { notes: [] };
      }
    },
    { shouldMockEvents: true },
  );
  return asked;
}

const asks = (asked: ReturnType<typeof core>, cmd: string) =>
  asked.filter((one) => one.cmd === cmd);

async function gradeAll(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByTestId("skill-library-select"));
  await screen.findByTestId("select-none");
  await user.keyboard("{Meta>}a{/Meta}");
  const page = screen.getByTestId("select-page");
  await user.click(await within(page).findByRole("button", { name: "Grade…" }));
  return screen.findByTestId("grades-review");
}

const SKILLS = [skill("deploy"), skill("lint"), skill("review"), skill("tidy")];

describe("Grade in the Library's Select mode", () => {
  it("shows the estimate before anything runs, says where it has none to go on, and names those with no tests", async () => {
    const asked = core(SKILLS, PLAN);
    renderStudio(AT_THE_ROOT);
    const user = userEvent.setup();
    const review = await gradeAll(user);
    expect(within(review).getByRole("heading", { name: "Grade 3 skills?" })).toBeTruthy();
    await waitFor(() =>
      expect(within(review).getByTestId("grades-estimate").textContent).toContain("About $0.40"),
    );
    const none = within(review).getByTestId("grades-unknown");
    expect(within(none).getByTestId("grades-unknown-lint").textContent).toContain(
      "not been graded here before",
    );
    expect(within(none).getByTestId("grades-unknown-review").textContent).toContain(
      "did not say what it spent",
    );
    expect(within(review).getByTestId("grades-left-out-tidy").textContent).toContain(
      "It has no tests.",
    );
    const members = asks(asked, "skill_grades_plan")[0]?.args.skills as [unknown, string][];
    expect(members.map(([, name]) => name)).toEqual(["deploy", "lint", "review", "tidy"]);
    expect(asks(asked, "skill_grades_start")).toHaveLength(0);
  });

  it("starts only once a limit is set, with each skill's consent, and ends Select", async () => {
    const asked = core(SKILLS, PLAN, {
      plane: PLANE,
      batch: "b1",
      limit_usd: 2,
      spent_usd: 0,
      skills: [],
      ended: null,
    });
    renderStudio(AT_THE_ROOT);
    const user = userEvent.setup();
    const review = await gradeAll(user);
    const start = await within(review).findByRole("button", { name: "Start grading" });
    // Some have nothing to go on, so purlis does not guess a limit for the person.
    expect(unavailable(start)).toBe(true);
    await user.type(within(review).getByRole("textbox", { name: "Spending limit" }), "2");
    await waitFor(() => expect(unavailable(start)).toBe(false));
    await user.click(start);
    await vi.waitFor(() => expect(asks(asked, "skill_grades_start")).toHaveLength(1));
    const args = asks(asked, "skill_grades_start")[0]?.args;
    expect(args?.limitUsd).toBe(2);
    expect((args?.skills as { name: string; consent: string }[]).map((one) => one.consent)).toEqual(
      ["c-deploy", "c-lint", "c-review"],
    );
    await screen.findByTestId("grades-running");
    expect(screen.queryByTestId("select-page")).toBeNull();
  });

  it("hears the limit stop it, and keeps the grades that ended", async () => {
    const at = (
      name: string,
      state: SkillGradesStep["skills"][number]["state"],
      more: Partial<SkillGradesStep["skills"][number]> = {},
    ) => ({
      level: PROJECT,
      name,
      state,
      grade: null,
      ran: 0,
      of: 6,
      cost_usd: null,
      ended: null,
      ...more,
    });
    const running: SkillGradesStep = {
      plane: PLANE,
      batch: "b1",
      limit_usd: 1,
      spent_usd: 0.6,
      skills: [
        at("deploy", { state: "ended" }, { grade: "g1", ran: 6, cost_usd: 0.6, ended: "whole" }),
        at("lint", { state: "running" }, { grade: "g2", ran: 2 }),
        at("review", { state: "running" }, { grade: "g3", ran: 1 }),
      ],
      ended: null,
    };
    core(SKILLS, { ...PLAN, unknown: [], known_usd: 1.5, known: 3 }, running);
    renderStudio(AT_THE_ROOT);
    const user = userEvent.setup();
    const review = await gradeAll(user);
    const start = await within(review).findByRole("button", { name: "Start grading" });
    await waitFor(() => expect(unavailable(start)).toBe(false));
    await user.click(start);
    const page = await screen.findByTestId("grades-running");
    await waitFor(() =>
      expect(within(page).getByTestId("grades-skill-lint").textContent).toContain(
        "Running: 2 of 6 chats",
      ),
    );
    expect(unavailable(within(page).getByRole("button", { name: "Stop" }))).toBe(false);
    await emit("skill-grades", {
      ...running,
      spent_usd: 1.1,
      skills: [
        running.skills[0],
        at("lint", { state: "ended" }, { grade: "g2", ran: 3, cost_usd: 0.5, ended: "stopped" }),
        at("review", { state: "not-run", why: "The limit was reached first." }),
      ],
      ended: "limit",
    } satisfies SkillGradesStep);
    await waitFor(() =>
      expect(within(page).getByTestId("grades-ended").textContent).toContain(
        "Stopped at your limit: $1.10 of $1.00. 2 grades ended",
      ),
    );
    expect(within(page).getByTestId("grades-skill-deploy").textContent).toContain("Graded, $0.60");
    expect(within(page).getByTestId("grades-skill-lint").textContent).toContain(
      "Stopped after 3 of 6 chats",
    );
    expect(within(page).getByTestId("grades-skill-review").textContent).toContain(
      "The limit was reached first.",
    );
    expect(unavailable(within(page).getByRole("button", { name: "Stop" }))).toBe(true);
  });
});

describe("what the estimate says", () => {
  it("adds what is known, and says when nothing is", () => {
    expect(estimateSaid({ ...PLAN, unknown: [], known: 3, known_usd: 1.234 })).toBe(
      "About $1.23 in all, from each skill's last grade.",
    );
    expect(estimateSaid(PLAN)).toBe(
      "About $0.40 for the one graded before. purlis has nothing to go on for the other 2.",
    );
    expect(estimateSaid({ ...PLAN, known: 0, known_usd: 0 })).toBe(
      "purlis has nothing to go on: none of these has a last grade that said what it spent.",
    );
  });
});

describe("Resume in the Library's Select mode (#5718)", () => {
  const one = (
    name: string,
    ended: SkillGradesStep["skills"][number]["ended"],
    grade: string | null,
    state: SkillGradesStep["skills"][number]["state"] = { state: "ended" },
  ): SkillGradesStep["skills"][number] => ({
    level: { at: "project", local: false },
    name,
    state,
    grade,
    ran: 3,
    of: 12,
    cost_usd: 0.2,
    ended,
  });

  it("offers to resume each grade that stopped short, and none that finished or never ran", () => {
    const step: SkillGradesStep = {
      plane: PLANE,
      batch: "b1",
      limit_usd: 2,
      spent_usd: 2.1,
      skills: [
        one("deploy", "whole", "g1"),
        one("lint", "stopped", "g2"),
        one("review", "usage-limit", "g3"),
        one("tidy", null, null, { state: "not-run", why: "The limit was reached first." }),
        one("crashed", "failed", "g5"),
      ],
      ended: "limit",
    };
    expect(stoppedShortOf(step)).toEqual(["g2", "g3"]);
    expect(stoppedShortOf(undefined)).toEqual([]);
  });

  it("says a resume's estimate is of what each left, at what its chats spent", () => {
    expect(resumeEstimateSaid({ known: 1, known_usd: 0.45, unknown: [] })).toBe(
      "About $0.45 in all, for what each has left, at what its chats spent so far.",
    );
  });
});
