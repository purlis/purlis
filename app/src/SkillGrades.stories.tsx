// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Grading a skill, as pages in the Skill studio** (#3688, `prototype/skill-grading` variant
 * A; its checks, `prototype/plain-checks` layout A): a skill's Grades tab with its checks, a card
 * per kind with its bars, its list and the after-each-edit row; one with no checks, one whose
 * checks a chat wrote and wait as a draft, one never graded, and one with only "picked" checks;
 * the plan with its totals first and Start fixed at its foot; a plan that changed
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
import {
  expectFocusInSight,
  tabInto,
  untilFocusInSight,
  untilFocusWithin,
} from "./stories.testkit";
import type {
  GradeRow,
  GradeScorecard,
  SkillDraft,
  SkillGradePlan,
  SkillGradeResumePlan,
} from "./bindings";

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
  draft: false,
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
  spent: {
    chats: 36,
    tokens: 300000,
    cost_usd: 1.92,
    limit_usd: 3,
    resumed: false,
    earlier_tokens: null,
    earlier_usd: null,
  },
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
    spent: {
      chats: 22,
      tokens: 240000,
      cost_usd: 1.71,
      limit_usd: 1.71,
      resumed: false,
      earlier_tokens: null,
      earlier_usd: null,
    },
  }),
  row("01A", {
    graded: "2026-10-20 08:00 UTC",
    version: "fedcba987654",
    ran_on: "Claude Code · haiku",
    outcome: "interrupted",
    ended: "Interrupted: not scored",
    arms: { with: null, without: null, previous: null },
    outcome_arms: null,
    spent: {
      chats: 2,
      tokens: null,
      cost_usd: 0.21,
      limit_usd: null,
      resumed: false,
      earlier_tokens: null,
      earlier_usd: null,
    },
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
  draft: false,
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
  resets_at: null,
  asked: 0,
  outcome: "graded",
  arms: { with: 86, without: 68, previous: 80 },
  spent: {
    chats: 36,
    tokens: 300000,
    cost_usd: 1.92,
    limit_usd: 3,
    resumed: false,
    earlier_tokens: null,
    earlier_usd: null,
  },
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
  delta: null,
  against: null,
  delta_without: null,
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
  /** How many outcome evals deploy has: none where it has only trigger tests. */
  evals?: number;
  /** The chats' drafts: one of deploy's checks waits on its Grades tab. */
  drafts?: SkillDraft[];
  /** What resuming the scorecard's grade would run (#5718): none where nothing is kept. */
  resume?: SkillGradeResumePlan | null;
}) {
  return <StudioInPanel width={width} height={620} narrow={narrow} onOpenView={() => undefined} />;
}

const meta = {
  component: Tab,
  args: { width: 1100, plan: PLAN, grades: GRADES, card: CARD },
  beforeEach: ({ args }) => {
    let plans = 0;
    // The grade running, at the step the app last told: none of its chats ran yet.
    const running = () =>
      args.plan.running === null
        ? null
        : {
            plane: PLANE,
            skill: "deploy",
            level: { at: "project" as const, local: false },
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
            draft: null,
          };
    mockIPC((cmd) => {
      switch (cmd) {
        case "skill_studio":
          return studio([DEPLOY], { workspace: "smart-ide", workspaces: ["smart-ide"] });
        case "skill_details":
          return {
            copies: [],
            uses: [],
            used_by: [],
            hinted_by: [],
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
            evals: args.evals ?? args.plan.evals.length,
            tests_refused: args.plan.tests_refused,
            running: running(),
          };
        case "skill_grade_running":
          return running();
        case "skill_drafts":
          return args.drafts ?? [];
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
        case "skill_grade_resume_plan":
          return args.resume ?? null;
        case "skill_grade_resume":
          return "01R";
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

/** deploy's grades, the latest of which ran its outcome evals too. */
const BOTH: GradeRow[] = GRADES.map((one, at) =>
  at === 0 ? { ...one, outcome_arms: { with: 75, without: 20, previous: null } } : one,
);

/**
 * **Checks, graded, both kinds** (ruling of 2026-11-02): the latest grade in one sentence, a card
 * per kind with its question, its technical name and its bars, every grade with how it ended, and
 * the after-each-edit row. Tab reaches the grades, and → walks the latest one to its Scorecard.
 */
export const Grades: Story = {
  args: { grades: BOTH, evals: 2 },
  play: async ({ canvas }) => {
    await expect(await canvas.findByTestId("skill-checks-answer")).toHaveTextContent(
      "With deploy, chats picked it right 86% of the time and met 75% of what the job asks; without it, 68% and 20%.",
    );
    const bars = await canvas.findByRole("group", {
      name: "Does it get picked? The latest grade of deploy",
    });
    await expect(within(bars).getAllByRole("meter")).toHaveLength(3);
    const job = canvas.getByRole("group", {
      name: "Does it do the job? The latest grade of deploy",
    });
    await expect(within(job).getAllByRole("meter")).toHaveLength(2);
    const list = await canvas.findByRole("grid", { name: "Grades of deploy" });
    await waitFor(() => expect(within(list).getAllByRole("row")).toHaveLength(4));
    const open = within(list).getByRole("button", {
      name: "Scorecard of the grade of 2026-10-28 10:00 UTC",
    });
    // The table is one stop for Tab: it lands on the latest grade's row. A second Tab leaves the
    // table: React Aria first moves the keyboard to its last control, the oldest grade's What ran,
    // below the fold, so the browser's Tab goes on from there. The story's simulated Tab went on
    // from the row it began on, to the Scorecard: a path no person takes, through a step below
    // what is pictured (#4098). Within the
    // row, → walks its cells to the Scorecard, each read on the frame after the press, which is
    // when a cell hands the keyboard to its control (React Aria's `useGridCell`).
    await tabInto(() => list);
    for (let step = 0; step < 12 && !open.contains(document.activeElement); step++) {
      await userEvent.keyboard("{ArrowRight}");
      await new Promise<void>((frame) => requestAnimationFrame(() => frame()));
    }
    await untilFocusWithin(() => open, "→ along the latest grade to its Scorecard");
    // The table's cell is scrolled in by React Aria a frame later, late in WebKit (#4026).
    await untilFocusInSight("→ to the Scorecard below the checks");
  },
};

/** deploy's plan with no checks at all. */
const NONE: SkillGradePlan = { ...PLAN, tests: [], chats: 0, after_edit_chats: 0, estimate: null };

/**
 * **A skill with no checks yet** (ruling of 2026-11-02): the tab says what the two kinds are,
 * offers nothing to run, and offers one chat to write both, or to write them yourself. Tab
 * reaches both.
 */
export const NoChecks: Story = {
  args: { plan: NONE, grades: [], evals: 0 },
  play: async ({ canvas }) => {
    const none = await canvas.findByTestId("skill-checks-none");
    await expect(none).toHaveTextContent("deploy has no checks yet");
    await expect(within(none).getByRole("button", { name: "Write them yourself" })).toBeVisible();
    const ask = within(none).getByRole("button", {
      name: "Ask a chat at plane to write checks",
    });
    await tabInto(() => ask);
    expectFocusInSight();
  },
};

/** One file of a draft: new, `after` its text. */
const drafted = (path: string, after: unknown) => ({
  path,
  before: null,
  after: JSON.stringify(after, null, 2),
  hunks: [],
  stale: false,
});

/** Chat 14's draft of deploy's checks: both kinds, in the skill's one draft. */
const CHECKS_DRAFT: SkillDraft = {
  id: "01D",
  name: "deploy",
  level: { at: "project", local: false },
  chat: 14,
  chat_open: true,
  new: false,
  stale: false,
  files: [
    drafted("evals/triggers.json", [
      { query: "deploy the api to staging", should_trigger: true },
      { query: "what changed since the last release?", should_trigger: false },
      { query: "ship svc to production with a tag", should_trigger: true },
      { query: "roll back yesterday's deploy", should_trigger: true },
    ]),
    drafted("evals/evals.json", {
      skill_name: "deploy",
      evals: [
        {
          id: 1,
          prompt: "Deploy svc to staging from a branch with a failing test",
          expectations: ["It stops before deploying", "It names the failing test"],
        },
        {
          id: 2,
          prompt: "Deploy the api to production",
          expectations: ["It runs the tests first", "It tags the release"],
        },
      ],
    }),
  ],
};

/**
 * **A chat wrote checks, and they wait** (ruling of 2026-11-02): both kinds in the skill's one
 * draft, each kind's checks under its question, accepted or discarded whole. Tab reaches Accept
 * checks.
 */
export const DraftWaiting: Story = {
  args: { plan: NONE, grades: [], evals: 0, drafts: [CHECKS_DRAFT] },
  play: async ({ canvas }) => {
    const draft = await canvas.findByTestId("skill-checks-draft");
    await expect(draft).toHaveTextContent("Chat 14 wrote checks for deploy");
    await expect(draft).toHaveTextContent("Does it get picked? · 4 prompts");
    await expect(draft).toHaveTextContent("Does it do the job? · 2 tasks");
    await expect(canvas.queryByTestId("skill-checks-none")).toBeNull();
    const accept = within(draft).getByRole("button", { name: "Accept checks" });
    await tabInto(() => accept);
    expectFocusInSight();
  },
};

/**
 * **Checks, never graded**: both kinds' cards, each saying how many it has and that it is not
 * graded yet, and Grade deploy… beside the heading. Tab reaches it.
 */
export const NeverGraded: Story = {
  args: { grades: [], evals: 2 },
  play: async ({ canvas }) => {
    const checks = await canvas.findByTestId("skill-checks");
    await expect(within(checks).getAllByText("Not graded yet.")).toHaveLength(2);
    const grade = within(checks).getByRole("button", { name: "Grade deploy…" });
    await tabInto(() => grade);
    expectFocusInSight();
  },
};

/**
 * **A skill with only “picked” checks**: its picked card with its bars, and the job's card saying
 * it has none yet, with a chat a press away. Tab reaches that offer.
 */
export const OnlyPicked: Story = {
  args: { evals: 0 },
  play: async ({ canvas }) => {
    await expect(await canvas.findByTestId("skill-checks-answer")).toHaveTextContent(
      "With deploy, chats picked it right 86% of the time; without it, 68%.",
    );
    const job = canvas.getByRole("region", { name: "Does it do the job?" });
    await expect(job).toHaveTextContent("None yet.");
    const ask = within(job).getByRole("button", {
      name: "Ask a chat at plane to write these",
    });
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

/** What resuming deploy's grade stopped at its cap would run (#5718). */
const RESUME: SkillGradeResumePlan = {
  grade: "01B",
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

/** A scorecard of a grade that stopped at its token cap, which can be resumed. */
const STOPPED_CARD: GradeScorecard = {
  ...CARD,
  grade: "01B",
  outcome: "at-limit",
  ended: "Stopped at the token cap",
};

/**
 * **A stopped grade offers Resume beside Grade again** (#5718): the press asks first, with the
 * chats left, about what they spend, the cap that holds them and what the stopped run spent.
 * Esc keeps everything as it was.
 */
export const ScorecardOffersResume: Story = {
  args: { grading: { at: "scorecard", grade: "01B" }, card: STOPPED_CARD, resume: RESUME },
  play: async ({ canvas }) => {
    const resume = await canvas.findByRole("button", { name: "Resume…" });
    await expect(canvas.getByRole("button", { name: "Grade again" })).toBeInTheDocument();
    await tabInto(() => resume);
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    const page = within(document.body);
    const question = await page.findByRole("alertdialog", {
      name: "Resume the grade of deploy?",
    });
    await expect(within(question).getByTestId("grade-resume-left")).toHaveTextContent(
      "About 40,000 tokens left to run; cap 200,000.",
    );
    await expect(within(question).getByTestId("grade-resume-earlier")).toHaveTextContent(
      "It spent 240,000 tokens, $1.71 so far.",
    );
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(page.queryByRole("alertdialog")).toBeNull());
  },
};

/** **A grade stopped at the program's usage limit**: Resume says when it can run, and waits. */
export const ScorecardResumeWaitsForTheLimit: Story = {
  args: {
    grading: { at: "scorecard", grade: "01B" },
    card: {
      ...STOPPED_CARD,
      outcome: "stopped",
      ended: "Stopped: Claude Code's usage limit was reached",
      resets_at: "2030-01-01T14:00:00Z",
    },
    resume: {
      ...RESUME,
      ended: "Stopped: Claude Code's usage limit was reached",
      waits_until: "2030-01-01T14:00:00Z",
    },
  },
  play: async ({ canvas }) => {
    const resume = await canvas.findByRole("button", {
      name: /^Resume after your limit resets at .*…$/,
    });
    await expect(resume).toHaveAttribute("aria-disabled", "true");
    await tabInto(() => resume);
    expectFocusInSight();
  },
};

/** **A grade whose skill changed since it stopped**: Grade again alone, saying what changed. */
export const ScorecardResumeNotOffered: Story = {
  args: {
    grading: { at: "scorecard", grade: "01B" },
    card: { ...STOPPED_CARD, outcome: "stopped", ended: "Stopped by you" },
    resume: {
      ...RESUME,
      ended: "Stopped by you",
      refused:
        "Resume is not offered: the skill's text changed since it stopped. Grade again runs it whole.",
      consent: null,
    },
  },
  play: async ({ canvas }) => {
    await expect(await canvas.findByTestId("grade-scorecard-resume-refused")).toHaveTextContent(
      "the skill's text changed since it stopped",
    );
    await expect(canvas.queryByRole("button", { name: "Resume…" })).toBeNull();
    await tabInto(() => canvas.getByRole("button", { name: "Grade again" }));
    expectFocusInSight();
  },
};

/** **A resumed grade's scorecard**: marked resumed, and its spend is both runs'. */
export const ScorecardOfAResumedGrade: Story = {
  args: {
    grading: { at: "scorecard", grade: "01C" },
    card: {
      ...CARD,
      spent: {
        ...CARD.spent,
        resumed: true,
        earlier_tokens: 240000,
        earlier_usd: 1.71,
        cost_usd: 2.01,
      },
    },
  },
  play: async ({ canvas }) => {
    await expect(await canvas.findByTestId("grade-scorecard-about")).toHaveTextContent(
      ", resumed,",
    );
    await expect(
      canvas.getByRole("group", { name: "Totals of the grade of deploy" }),
    ).toHaveTextContent("both runs · this run $0.30");
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

/** **The Grades tab of a grade of outcome evals alone**: the job's bars and its columns (SK-55). */
export const GradesOfOutcomeEvals: Story = {
  args: {
    evals: 1,
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
      await canvas.findByRole("group", { name: "Does it do the job? The latest grade of deploy" }),
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
