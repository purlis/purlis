// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Grading a skill, as pages in the Skill studio** (#3688, `prototype/skill-grading` variant
 * A): a skill's Grades tab with its bars, its list and the after-each-edit row; one with no
 * trigger tests; the plan with its totals first and Start fixed at its foot; a plan that changed
 * under the person, Start waiting; a grade running with Stop; a scorecard with its end banner,
 * totals and a row per test; and a saved chat, read-only. Walked by the keyboard, held to axe,
 * and pictured in every pictured theme cell. The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  STORY_HISTORY as HISTORY,
  STORY_PLANE as PLANE,
  StudioInPanel,
} from "./skillStudioStory.testkit";
import { inWorkspace, skillRowId, type GradingAt } from "./skillStudio";
import { skill, studio } from "./skillsListing.testkit";
import { forgetViewHistory, openViewAt } from "./ui/panel/viewHistory";
import { expectFocusInSight, untilFocusWithin } from "./stories.testkit";
import type { GradeRow, GradeScorecard, SkillGradePlan } from "./bindings";

const SCOPE = inWorkspace("smart-ide");

const DEPLOY = skill("deploy", {
  description:
    "Deploys a service to staging or production with the checks the team agreed on: tests green, a changelog entry and a tag.",
});

const TESTS = [
  { query: "deploy the api to staging", should_trigger: true, hides: false },
  { query: "what changed since the last release?", should_trigger: false, hides: false },
  { query: "ship svc to production with a tag", should_trigger: true, hides: false },
  { query: "roll back yesterday's deploy", should_trigger: true, hides: false },
];

const PLAN: SkillGradePlan = {
  tests: TESTS,
  chats: 36,
  runs: 3,
  after_edit_chats: 36,
  tests_refused: null,
  left_out: [],
  evals: [],
  outcome_evals: 0,
  judge_chats: 0,
  outcomes_left_out: [],
  outcomes_refused: null,
  estimate: {
    tokens: 300000,
    cost_usd: 1.92,
    chats_then: 36,
    ts: "2026-10-28T10:00:00+00:00",
    profile: "work",
    harness: "Claude Code",
    model: "sonnet",
    same_profile: true,
    interrupted: false,
  },
  record_said: null,
  running: null,
  token_cap: 400000,
  cap_set_here: true,
  cost_cap_usd: 3,
  priced: { usd_per_million: 7.5, of_the_model: true, model: "sonnet" },
  chat_minutes: 5,
  profile: "work",
  models: ["haiku", "sonnet", "opus"],
  model: "sonnet",
  sandbox: "full sandbox",
  refused: null,
  consent: "c0ffee",
  previous: { commit: "fedcba987654", subject: "First version" },
  previous_refused: null,
  graders: [],
  grader: null,
  grader_refused: null,
};

const row = (grade: string, over: Partial<GradeRow> = {}): GradeRow => ({
  grade,
  graded: "2026-10-28 10:00 UTC",
  version: "0123456789ab",
  ran_on: "Claude Code · sonnet",
  ended: "Finished",
  outcome: "graded",
  arms: { with: 86, without: 68, previous: 80 },
  outcome_arms: null,
  spent: { chats: 36, tokens: 300000, cost_usd: 1.92, limit_usd: 3 },
  after_edit: false,
  ...over,
});

const GRADES: GradeRow[] = [
  row("01C"),
  row("01B", {
    graded: "2026-10-22 09:00 UTC",
    version: "fedcba987654",
    outcome: "at-limit",
    ended: "Stopped at its dollar limit",
    arms: { with: 80, without: 69, previous: null },
    outcome_arms: null,
    spent: { chats: 22, tokens: 240000, cost_usd: 1.71, limit_usd: 1.71 },
  }),
  row("01A", {
    graded: "2026-10-20 08:00 UTC",
    version: "fedcba987654",
    ran_on: "Claude Code · haiku",
    outcome: "interrupted",
    ended: "Interrupted: not scored",
    arms: { with: null, without: null, previous: null },
    outcome_arms: null,
    spent: { chats: 2, tokens: null, cost_usd: 0.21, limit_usd: null },
  }),
];

const arm = (
  which: "with" | "without" | "previous",
  triggered: number,
  pass: boolean,
  first: number,
) => ({
  arm: which,
  runs: 3,
  triggered,
  pass,
  asked: 0,
  chats: [1, 2, 3].map((run) => ({ id: `01K${first + run}`, run, kept: true })),
});

const CARD: GradeScorecard = {
  grade: "01C",
  skill: "deploy",
  level: { at: "project", local: false },
  graded: "2026-10-28 10:00 UTC",
  version: "0123456789ab",
  ran_on: "Claude Code · sonnet",
  ended: "Finished",
  delta: 6,
  against: "fedcba987654",
  delta_without: 18,
  after_edit: false,
  asked: 0,
  outcome: "graded",
  arms: { with: 86, without: 68, previous: 80 },
  spent: { chats: 36, tokens: 300000, cost_usd: 1.92, limit_usd: 3 },
  outcome_arms: null,
  evals: [],
  tests: TESTS.map((test, at) => ({
    query: test.query,
    should_trigger: test.should_trigger,
    arms: [
      arm("with", test.should_trigger ? 3 : 0, true, at * 10),
      arm("without", at === 0 ? 3 : 1, at === 1, at * 10 + 3),
      arm("previous", test.should_trigger ? 2 : 0, at !== 2, at * 10 + 6),
    ],
  })),
};

/** A grade of outcome evals alone: one run failed by a judge's answer, one by what its chat asked. */
const EVALS_CARD: GradeScorecard = {
  ...CARD,
  tests: [],
  arms: { with: null, without: null, previous: null },
  outcome_arms: { with: 75, without: 0, previous: null },
  evals: [
    {
      id: "4",
      expectations: ["The form's name is filled in", "The date is written as ISO 8601"],
      arms: [
        {
          arm: "with",
          runs: 2,
          points: 75,
          results: [1, 2].map((run) => ({
            run,
            chat: { id: `01E${run}`, run, kept: true },
            judge: { id: `01J${run}`, run, kept: true },
            passed: run === 1 ? 2 : 1,
            checks: [
              { passed: true, evidence: "form.txt names Ada Lovelace" },
              { passed: run === 1, evidence: run === 1 ? "2026-10-28" : "it says 28/10/26" },
            ],
            failed: null,
          })),
        },
        {
          arm: "without",
          runs: 1,
          points: 0,
          results: [
            {
              run: 1,
              chat: { id: "01E3", run: 1, kept: true },
              judge: null,
              passed: 0,
              checks: [],
              failed:
                "Its chat asked for the tool Bash, which a grade's chat may not do on its own, so purlis ended it. That is a limit of grading, not a fault of the skill, and the run counts as failed.",
            },
          ],
        },
      ],
    },
  ],
};

const SCREEN =
  "> deploy the api to staging\r\n\r\n\u001b[1m● Skill(deploy)\u001b[0m\r\n  ⎿ Loaded deploy\r\n\r\nI'll check the tests are green first.\r\n● Bash(cargo test --quiet)\r\n  ⎿ test result: ok. 312 passed\r\n\r\n[ended by the grade: the skill started, which is what this test checks]\r\n";

/** The studio tab at deploy's Grades, or a grading page under it, `width` by 620 px. */
function Tab({
  width,
  narrow,
}: {
  width: number;
  narrow?: boolean;
  grading?: GradingAt;
  plan: SkillGradePlan;
  grades: GradeRow[];
  card: GradeScorecard;
}) {
  return <StudioInPanel width={width} height={620} narrow={narrow} onOpenView={() => undefined} />;
}

const meta = {
  component: Tab,
  args: { width: 1100, plan: PLAN, grades: GRADES, card: CARD },
  beforeEach: ({ args }) => {
    let plans = 0;
    mockIPC((cmd) => {
      switch (cmd) {
        case "skill_studio":
          return studio([DEPLOY], { workspace: "smart-ide", workspaces: ["smart-ide"] });
        case "skill_details":
          return {
            copies: [],
            uses: [],
            used_by: [],
            uses_cut: false,
            pinned_by: [],
            pins_refused: [],
          };
        case "skill_stats":
          return { by_harness: [], by_persona: [], by_workspace: [] };
        case "skill_grades":
          return {
            grades: args.grades,
            said: null,
            tests: args.plan.tests.length,
            evals: args.plan.evals.length,
            tests_refused: args.plan.tests_refused,
            // The grade running, at the step the app last told: none of its chats ran yet.
            running:
              args.plan.running === null
                ? null
                : {
                    plane: PLANE,
                    skill: "deploy",
                    grade: args.plan.running,
                    ran: 0,
                    of: args.plan.chats,
                    tokens: null,
                    cost_usd: null,
                    limit_usd: args.plan.cost_cap_usd,
                    cap: args.plan.token_cap,
                    asked: 0,
                    ended: null,
                    said: null,
                  },
          };
        case "skill_grade_plan":
          plans += 1;
          // A plan that changed under the person: the second read consents to another.
          return plans > 1 && args.plan.consent === "stale"
            ? { ...args.plan, consent: "f00d", model: "opus", cost_cap_usd: 9.6 }
            : args.plan;
        case "skill_grade_start":
          if (args.plan.consent === "stale")
            throw "The plan of deploy changed since it was shown, so nothing was started: read the new plan and press again.";
          return "01D";
        case "skill_grade_after_edit":
          return null;
        case "start_options":
          return { profiles: [{ name: "work" }] };
        case "skill_grade_scorecard":
          return args.card;
        case "skill_grade_chat":
          return SCREEN;
        case "skill_updates":
          return { offers: [], unconfirmed: [], unasked: [] };
        case "skill_deletes_cut_short":
          return { settled: [], unsettled: [] };
        default:
          return [];
      }
    });
    openViewAt(HISTORY, {
      page: "library",
      scope: SCOPE,
      skill: skillRowId(DEPLOY),
      tab: "grades",
      ...(args.grading === undefined ? {} : { grading: args.grading }),
    });
    return () => {
      clearMocks();
      forgetViewHistory(HISTORY);
    };
  },
} satisfies Meta<typeof Tab>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Tab, until the keyboard is inside `target`, at most `steps` times. */
async function tabInto(target: () => Element | null, steps = 60): Promise<void> {
  for (let step = 0; step < steps; step++) {
    const into = target();
    if (into !== null && into.contains(document.activeElement)) return;
    await userEvent.tab();
  }
  await untilFocusWithin(target, "the keyboard reaches it by Tab");
}

/**
 * **The Grades tab**: the latest grade as bars, every grade with how it ended, and the
 * after-each-edit row. Tab reaches a grade's Scorecard.
 */
export const Grades: Story = {
  play: async ({ canvas }) => {
    const bars = await canvas.findByRole("group", { name: "The latest grade of deploy" });
    await expect(within(bars).getAllByRole("meter")).toHaveLength(3);
    const list = await canvas.findByRole("grid", { name: "Grades of deploy" });
    await waitFor(() => expect(within(list).getAllByRole("row")).toHaveLength(4));
    const open = within(list).getByRole("button", {
      name: "Scorecard of the grade of 2026-10-28 10:00 UTC",
    });
    await tabInto(() => open);
    expectFocusInSight();
  },
};

/**
 * **A skill with no trigger tests**: the tab says what they are, offers nothing to run, and offers
 * to write them or to ask a chat to (#3925). Tab reaches both.
 */
export const NoTests: Story = {
  args: { plan: { ...PLAN, tests: [], chats: 0, after_edit_chats: 0, estimate: null }, grades: [] },
  play: async ({ canvas }) => {
    const none = await canvas.findByTestId("skill-grades-no-tests");
    await expect(none).toHaveTextContent("deploy has no trigger tests yet");
    await expect(within(none).getByRole("button", { name: "Write trigger tests" })).toBeVisible();
    const ask = within(none).getByRole("button", { name: "Ask a chat to write them" });
    await tabInto(() => ask);
    expectFocusInSight();
  },
};

/**
 * **The plan**: its totals first, then what it runs, where and what it may spend, with Start
 * fixed at its foot saying what it may spend of whose account.
 */
export const Plan: Story = {
  args: { grading: { at: "plan" } },
  play: async ({ canvas }) => {
    await canvas.findByRole("group", { name: "What a grade of deploy runs and spends" });
    const start = canvas.getByRole("button", { name: "Start grading · 36 chats" });
    await expect(start).toHaveAccessibleDescription(
      "Nothing runs until you press Start. It spends at most $3.00 of work's account.",
    );
    await tabInto(() => start);
    expectFocusInSight();
  },
};

/** **A plan that changed under the person**: said in the attention tone, Start waiting. */
export const PlanChanged: Story = {
  args: { grading: { at: "plan" }, plan: { ...PLAN, consent: "stale" } },
  play: async ({ canvas }) => {
    const start = await canvas.findByRole("button", { name: "Start grading · 36 chats" });
    await userEvent.click(start);
    await expect(await canvas.findByTestId("skill-grade-plan-changed")).toHaveTextContent(
      "it now runs 36 chats on work, opus, and may spend at most $9.60",
    );
    await expect(start).toHaveAttribute("aria-disabled", "true");
  },
};

/** **A plan a Grader extension scores** (#3236): named plainly, beside purlis's own. */
export const PlanByAGrader: Story = {
  args: {
    grading: { at: "plan" },
    plan: {
      ...PLAN,
      graders: [{ id: "acme.grader", title: "Acme evals", hosts: ["evals.acme.example"] }],
      grader: { id: "acme.grader", title: "Acme evals", hosts: ["evals.acme.example"] },
    },
  },
  play: async ({ canvas }) => {
    await expect(await canvas.findByTestId("skill-grade-grader")).toHaveTextContent(
      "Acme evals (acme.grader) is an extension you installed, not part of purlis.",
    );
  },
};

/** **A grade running**: its chats, its spend against its limit and its tokens, with Stop. */
export const Running: Story = {
  args: { grading: { at: "running", grade: "01D" }, plan: { ...PLAN, running: "01D" } },
  play: async ({ canvas }) => {
    await canvas.findByRole("group", { name: "How far the grade of deploy is" });
    const stop = canvas.getByRole("button", { name: "Stop" });
    await tabInto(() => stop);
    expectFocusInSight();
  },
};

/** **A scorecard**: how the grade ended, its totals, and a row per test with each chat. */
export const Scorecard: Story = {
  args: { grading: { at: "scorecard", grade: "01C" } },
  play: async ({ canvas }) => {
    await expect(await canvas.findByTestId("grade-scorecard-about")).toHaveTextContent(
      "Graded 2026-10-28 10:00 UTC",
    );
    const tests = await canvas.findByRole("grid", { name: "Trigger tests of deploy" });
    await waitFor(() => expect(within(tests).getAllByRole("row")).toHaveLength(5));
  },
};

/** **A scorecard of a grade that stopped at its limit**, in the attention tone. */
export const ScorecardAtLimit: Story = {
  args: {
    grading: { at: "scorecard", grade: "01B" },
    card: { ...CARD, grade: "01B", outcome: "at-limit", ended: "Stopped at its dollar limit" },
  },
  play: async ({ canvas }) => {
    await expect(await canvas.findByTestId("grade-scorecard-about")).toHaveTextContent(
      "Stopped at its limit",
    );
  },
};

/** **A scorecard of outcome evals**: each arm's rate, each run's chat and judge, why a run failed,
 *  and what the judges found of each expectation (SK-55). Tab reaches a run's chat. */
export const ScorecardOfOutcomeEvals: Story = {
  args: { grading: { at: "scorecard", grade: "01C" }, card: EVALS_CARD },
  play: async ({ canvas }) => {
    await expect(await canvas.findByTestId("grade-scorecard-eval-why")).toHaveTextContent(
      "Its chat asked for the tool Bash",
    );
    const evals = await canvas.findByRole("grid", { name: "Outcome evals of deploy" });
    await expect(within(evals).getAllByRole("row")).toHaveLength(2);
    await expect(
      canvas.getByRole("group", { name: "Outcome evals of the grade of deploy" }),
    ).toHaveTextContent("75%");
  },
};

/** **The Grades tab of a grade of outcome evals**: its bars and its column (SK-55). */
export const GradesOfOutcomeEvals: Story = {
  args: {
    grades: GRADES.map((row, at) =>
      at === 0
        ? {
            ...row,
            arms: { with: null, without: null, previous: null },
            outcome_arms: { with: 75, without: 0, previous: null },
          }
        : row,
    ),
  },
  play: async ({ canvas }) => {
    await expect(
      await canvas.findByRole("group", { name: "Outcome evals of the latest grade of deploy" }),
    ).toBeInTheDocument();
    await expect(canvas.queryByText(/nothing of deploy was compared/)).toBeNull();
  },
};

/** **A saved chat**, read-only, with the grade's other kept chats a press away. */
export const SavedChat: Story = {
  args: { grading: { at: "chat", grade: "01C", chat: "01K1" } },
  play: async ({ canvas }) => {
    await canvas.findByText(/It takes no input/);
    const back = canvas.getByRole("button", { name: "Back to the scorecard" });
    await tabInto(() => back);
    expectFocusInSight();
  },
};

/** **The plan, narrow**: the page alone, its summary over the answers, nothing sideways. */
export const PlanNarrow: Story = {
  args: { width: 460, narrow: true, grading: { at: "plan" } },
  play: async ({ canvas, canvasElement }) => {
    await canvas.findByRole("group", { name: "What a grade of deploy runs and spends" });
    const root = canvasElement.querySelector<HTMLElement>('[data-part="root"][data-narrow]');
    if (root === null) throw new Error("the frame is not narrow");
    await expect(root.scrollWidth).toBeLessThanOrEqual(root.clientWidth + 1);
  },
};
