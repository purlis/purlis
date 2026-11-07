// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Grade, in the Library's Select mode** (#5356): the estimate in the selection page's place,
 * from each skill's last grade, naming those it has nothing to go on for and those with no tests;
 * the limit the person sets; and the grades running a few at a time, then stopped at the limit.
 * Walked by the keyboard (Select, ⌘A, Grade, the limit, Start), held to axe, and pictured in every
 * pictured theme cell. The core is faked at its command boundary: no grade runs.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { SkillGradesPlan, SkillGradesStep, Studio } from "./bindings";
import { STORY_HISTORY as HISTORY, StudioInPanel } from "./skillStudioStory.testkit";
import { AT_THE_ROOT } from "./skillStudio";
import { skill, studio } from "./skillsListing.testkit";
import { expectFocusInSight, tabInto } from "./stories.testkit";
import { forgetViewHistory, openViewAt } from "./ui/panel/viewHistory";

const PROJECT = { at: "project", local: false } as const;

const SKILLS = [
  skill("deploy", { description: "Deploys a service to staging or production." }),
  skill("grilling", { description: "Grills the person about a plan until the gaps show." }),
  skill("tdd", { description: "Test-driven development: red, green, refactor." }),
  skill("scratch-notes", { description: "Notes kept while working." }),
  skill("purlis-browser", {
    level: "built-in",
    label: "built-in",
    grant: null,
    granted: null,
    description: "Drives a real browser for a task that needs one.",
  }),
];

const lastGrade = (cost_usd: number | null) => ({
  tokens: cost_usd === null ? null : 40_000,
  cost_usd,
  chats_then: 12,
  ts: "2026-11-01T10:00:00Z",
  profile: "work",
  harness: "claude",
  model: null,
  same_profile: true,
  interrupted: false,
});

const PLAN: SkillGradesPlan = {
  skills: [
    {
      level: PROJECT,
      name: "deploy",
      chats: 12,
      limit_usd: 3,
      estimate: lastGrade(0.84),
      consent: "a",
      resume: null,
    },
    {
      level: PROJECT,
      name: "grilling",
      chats: 18,
      limit_usd: 3,
      estimate: null,
      consent: "b",
      resume: null,
    },
    {
      level: PROJECT,
      name: "tdd",
      chats: 12,
      limit_usd: 3,
      estimate: lastGrade(0.61),
      consent: "c",
      resume: null,
    },
  ],
  left_out: [{ level: PROJECT, name: "scratch-notes", why: "It has no tests." }],
  known_usd: 1.45,
  known: 2,
  unknown: ["grilling"],
  most_usd: 9,
  at_once: 3,
  profile: "work",
  refused: null,
  running: null,
};

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
  of: 12,
  cost_usd: null,
  ended: null,
  ...more,
});

const RUNNING: SkillGradesStep = {
  plane: "story-plane",
  batch: "b1",
  limit_usd: 2,
  spent_usd: 0.9,
  skills: [
    at("deploy", { state: "ended" }, { grade: "g1", ran: 12, cost_usd: 0.8, ended: "whole" }),
    at("grilling", { state: "running" }, { grade: "g2", ran: 4, of: 18, cost_usd: 0.1 }),
    at("tdd", { state: "running" }, { grade: "g3", ran: 1 }),
  ],
  ended: null,
};

const AT_THE_LIMIT: SkillGradesStep = {
  ...RUNNING,
  spent_usd: 2.07,
  skills: [
    RUNNING.skills[0],
    at(
      "grilling",
      { state: "ended" },
      { grade: "g2", ran: 9, of: 18, cost_usd: 0.9, ended: "stopped" },
    ),
    at("tdd", { state: "not-run", why: "The limit was reached first." }),
  ],
  ended: "limit",
};

/** Resuming grilling's grade, stopped at the batch's limit, under one new limit (#5718). */
const RESUME_PLAN: SkillGradesPlan = {
  ...PLAN,
  skills: [
    {
      level: PROJECT,
      name: "grilling",
      chats: 9,
      limit_usd: 1.5,
      estimate: { ...lastGrade(0.45), chats_then: 9 },
      consent: "r",
      resume: "g2",
    },
  ],
  left_out: [],
  known_usd: 0.45,
  known: 1,
  unknown: [],
  most_usd: 1.5,
};

function Tab({ width }: { width: number; running: SkillGradesStep; answer: Studio }) {
  return <StudioInPanel width={width} height={560} />;
}

const meta = {
  component: Tab,
  args: { width: 1000, running: RUNNING, answer: studio(SKILLS) },
  beforeEach: ({ args }) => {
    let started = false;
    mockIPC((cmd, given) => {
      if (cmd === "skill_studio") return args.answer;
      if (cmd === "skill_deletes_cut_short") return { settled: [], unsettled: [] };
      if (cmd === "skill_grades_plan") return PLAN;
      if (cmd === "skill_grades_resume_plan") return RESUME_PLAN;
      if (cmd === "skill_grades_start") {
        started = true;
        return "b1";
      }
      if (cmd === "skill_grades_running") {
        const plane = (given as { plane: string }).plane;
        return started ? { ...args.running, plane } : null;
      }
      return [];
    });
    openViewAt(HISTORY, { page: "library", scope: AT_THE_ROOT });
    return () => {
      clearMocks();
      forgetViewHistory(HISTORY);
    };
  },
} satisfies Meta<typeof Tab>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Select, ⌘A, then Grade from the keyboard: its review, in the selection page's place. */
async function review(canvas: ReturnType<typeof within>) {
  await canvas.findByRole("grid", { name: "Skills" });
  await tabInto(() => canvas.getByTestId("skill-library-select"));
  await userEvent.keyboard("{Enter}");
  await canvas.findByTestId("select-none");
  await tabInto(() => canvas.getByRole("grid", { name: "Skills" }));
  await userEvent.keyboard("{Meta>}a{/Meta}");
  const page = await canvas.findByTestId("select-page");
  await tabInto(() => within(page).getByRole("button", { name: "Grade…" }));
  await userEvent.keyboard("{Enter}");
  const shown = await canvas.findByTestId("grades-review");
  await within(shown).findByTestId("grades-estimate");
  return shown;
}

/** Sets the limit from the keyboard and starts: the grades' page. */
async function start(canvas: ReturnType<typeof within>) {
  const shown = await review(canvas);
  await tabInto(() => within(shown).getByRole("textbox", { name: "Spending limit" }));
  await userEvent.keyboard("2");
  await waitFor(() =>
    expect(within(shown).getByRole("button", { name: "Start grading" })).not.toHaveAttribute(
      "aria-disabled",
      "true",
    ),
  );
  await tabInto(() => within(shown).getByRole("button", { name: "Start grading" }));
  await userEvent.keyboard("{Enter}");
  return canvas.findByTestId("grades-running");
}

/**
 * **The estimate**: about what the two graded before spent, the one with nothing to go on named,
 * the one with no tests and the built-in left out. Start waits for a limit.
 */
export const Estimate: Story = {
  play: async ({ canvas }) => {
    const shown = await review(canvas);
    await expect(within(shown).getByTestId("grades-estimate")).toHaveTextContent("About $1.45");
    await expect(within(shown).getByTestId("grades-unknown-grilling")).toBeVisible();
    await expect(within(shown).getByTestId("grades-left-out-scratch-notes")).toHaveTextContent(
      "It has no tests.",
    );
    await expect(within(shown).getByRole("button", { name: "Start grading" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    await tabInto(() => within(shown).getByRole("textbox", { name: "Spending limit" }));
    expectFocusInSight();
  },
};

/** **Running, a few at a time**: one graded, two running, what they spent against the limit. */
export const Running: Story = {
  play: async ({ canvas }) => {
    const page = await start(canvas);
    await waitFor(() =>
      expect(within(page).getByTestId("grades-skill-grilling")).toHaveTextContent("Running"),
    );
    await tabInto(() => within(page).getByRole("grid", { name: "Each skill" }));
    expectFocusInSight();
  },
};

/** **Stopped at the limit**: the grades that ended kept, the one never started named. */
export const StoppedAtTheLimit: Story = {
  args: { running: AT_THE_LIMIT },
  play: async ({ canvas }) => {
    const page = await start(canvas);
    await waitFor(() =>
      expect(within(page).getByTestId("grades-ended")).toHaveTextContent("Stopped at your limit"),
    );
    await expect(within(page).getByTestId("grades-skill-tdd")).toHaveTextContent(
      "The limit was reached first.",
    );
    await tabInto(() => within(page).getByRole("button", { name: "Back to the Library" }));
    expectFocusInSight();
  },
};

/**
 * **A batch that stopped short offers Resume** (#5718): the grade the limit stopped is resumed
 * under one new limit, from a review that counts only the chats it left.
 */
export const ResumeTheStopped: Story = {
  args: { running: AT_THE_LIMIT },
  play: async ({ canvas }) => {
    const page = await start(canvas);
    const resume = await within(page).findByRole("button", { name: "Resume them…" });
    await tabInto(() => resume);
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    const shown = await canvas.findByTestId("grades-review");
    await within(shown).findByTestId("grades-estimate");
    await expect(within(shown).getByTestId("grades-going-grilling")).toHaveTextContent(
      "9 chats left",
    );
    await expect(within(shown).getByRole("button", { name: "Resume them" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    await tabInto(() => within(shown).getByRole("textbox", { name: "Spending limit" }));
    expectFocusInSight();
  },
};
