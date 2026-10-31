// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Skill studio's Harness skills and Routing pages** (#3692, `prototype/studio-secondary-pages`
 * variant A): each a list with the item chosen beside it. Harness skills groups each program's
 * own skills, with whose each is and its switch beside the list, or why a plugin's is read-only;
 * Routing says the router with Change…, the budget, and each chat's start. Walked by the
 * keyboard, held to axe, and pictured in every pictured theme cell. The core is faked at its
 * command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { STORY_HISTORY as HISTORY, StudioInPanel } from "./skillStudioStory.testkit";
import { inWorkspace, routerGradeItem, type StudioPage, type StudioScope } from "./skillStudio";
import { skill, studio } from "./skillsListing.testkit";
import { forgetViewHistory, openViewAt } from "./ui/panel/viewHistory";
import { expectFocusInSight, untilFocusWithin } from "./stories.testkit";
import type { RouterGradeScorecard, ScoredTest, Studio, StudioRouter } from "./bindings";

/** The prototype's programs: Claude Code's own, a plugin's and a repo's; Codex none; opencode's
 *  one, off here. */
const HARNESSES: Studio["harnesses"] = [
  {
    harness: "claude",
    title: "Claude Code",
    repo_left_out: [],
    own: null,
    skills: [
      skill("my-notes", {
        level: "installed",
        label: "installed",
        grant: null,
        granted: null,
        description: "Keeps your running notes.",
        folder: "/home/dev/.claude/skills/my-notes",
        owner: { kind: "you" },
        switch: { kind: "alone" },
      }),
      skill("frontend-design:frontend-design", {
        level: "installed",
        label: "installed",
        grant: null,
        granted: null,
        description: "Builds distinctive front-end pages.",
        folder: "/cache/frontend-design/skills/frontend-design",
        owner: { kind: "plugin", name: "frontend-design" },
        switch: {
          kind: "not",
          why: "it comes with the plugin frontend-design, which Claude Code turns on or off itself, so its skills are shown read-only",
        },
      }),
      skill("lint-ts", {
        level: "repo",
        label: "repo",
        grant: null,
        granted: null,
        description: "Runs eslint the way CI does.",
        folder: "workspaces/smart-ide/svc/.claude/skills/lint-ts",
        owner: { kind: "repo", name: "svc" },
        switch: { kind: "alone" },
      }),
    ],
  },
  { harness: "codex", title: "Codex", repo_left_out: [], own: null, skills: [] },
  {
    harness: "opencode",
    title: "opencode",
    repo_left_out: [],
    own: null,
    skills: [
      skill("scratch", {
        level: "installed",
        label: "installed",
        grant: null,
        granted: null,
        on: false,
        left_out: "turned off for smart-ide's chats",
        description: "A scratch skill.",
        folder: "/home/dev/.config/opencode/skills/scratch",
        owner: { kind: "you" },
        switch: { kind: "alone" },
      }),
    ],
  },
];

/** An extension's router, with two chats' starts: one quick, one that fell back. */
const ROUTER: StudioRouter = {
  said: "Chats here are routed by the router relevance, asked at each chat's start.",
  problem: false,
  extension: "relevance",
  written: "relevance",
  budget: { skills: 30, chars: 6000 },
  grade:
    "Graded on the library at 3f9a2c1d7e40, 2026-10-27: 9 of 10 trigger tests pass with the router relevance, and 8 of 10 with the built-in router.",
  graded: "01JBGRADE0000000000000000",
  chats: [
    {
      session: 5,
      routing: {
        router: "relevance",
        ms: 41,
        fell_back: false,
        said: "Routed by the router relevance in 41 ms.",
      },
      hidden: ["figma-handoff", "gpu-profile", "lint-rs"],
    },
    {
      session: 2,
      routing: {
        router: "relevance",
        ms: 112,
        fell_back: true,
        said: "The router relevance did not answer within 100 ms, so this chat lists what the built-in router shows.",
      },
      hidden: [],
    },
  ],
};

/** One trigger test of a router's grade, three runs in each arm, each chat kept. */
function scored(query: string, routerTriggered: number, builtIn: number): ScoredTest {
  const runs = (arm: "router" | "built-in-router", triggered: number) => ({
    arm,
    runs: 3,
    triggered,
    pass: triggered >= 2,
    asked: 0,
    chats: [1, 2, 3].map((run) => ({ id: `${arm}-${query.length}-${run}`, run, kept: true })),
  });
  return {
    query,
    should_trigger: true,
    arms: [runs("router", routerTriggered), runs("built-in-router", builtIn)],
  };
}

/** The router's newest grade's scorecard (#3617): two skills, both arms. */
const SCORECARD: RouterGradeScorecard = {
  grade: "01JBGRADE0000000000000000",
  router: "relevance",
  graded: "2026-10-27 09:12 UTC",
  library: "the library at 3f9a2c1d7e40",
  ran_on: "Claude Code · claude-sonnet-4-5",
  ended: "Finished",
  outcome: "graded",
  arms: { router: 90, built_in: 80 },
  spent: { chats: 60, tokens: 240000, cost_usd: 1.84, limit_usd: 15 },
  asked: 0,
  skills: [
    {
      skill: "figma-handoff",
      tests: [
        scored("turn this frame into a react component", 3, 3),
        scored("export the figma tokens", 2, 1),
      ],
    },
    { skill: "lint-rs", tests: [scored("why does clippy fail here", 3, 2)] },
  ],
};

/** The studio tab as a view tab draws it, `width` by 520 px, opened at `page` on `scope`, at
 *  `item` on it where one is named. */
function Tab({
  width,
  narrow,
}: {
  width: number;
  narrow?: boolean;
  page: StudioPage;
  scope: StudioScope;
  answer: Studio;
  item?: string;
}) {
  return <StudioInPanel width={width} height={520} narrow={narrow} />;
}

const meta = {
  component: Tab,
  args: {
    width: 1000,
    page: "harness",
    scope: inWorkspace("smart-ide"),
    answer: studio([], {
      workspace: "smart-ide",
      workspaces: ["smart-ide"],
      harnesses: HARNESSES,
      router: ROUTER,
    }),
  },
  beforeEach: ({ args }) => {
    mockIPC((cmd) => {
      if (cmd === "skill_studio") return args.answer;
      if (cmd === "skill_routers") return [{ id: "relevance", name: "Relevance router" }];
      if (cmd === "skill_deletes_cut_short") return { settled: [], unsettled: [] };
      if (cmd === "router_grade_scorecard") return SCORECARD;
      return [];
    });
    openViewAt(HISTORY, {
      page: args.page,
      scope: args.scope,
      ...(args.item === undefined ? {} : { item: args.item }),
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
async function tabInto(target: () => Element | null, steps = 20): Promise<void> {
  for (let step = 0; step < steps; step++) {
    const into = target();
    if (into !== null && into.contains(document.activeElement)) return;
    await userEvent.tab();
  }
  await untilFocusWithin(target, "the keyboard reaches it by Tab");
}

/**
 * **Harness skills**: each program's own skills under its name, a plugin's read-only, a program
 * with none saying so. ↓ and Enter choose a skill, which opens beside the list with its switch,
 * which the keyboard reaches.
 */
export const HarnessSkills: Story = {
  play: async ({ canvas }) => {
    const grid = await canvas.findByRole("grid", { name: "Each program's own skills" });
    await expect(within(grid).getByText("Codex has no skills of its own here.")).toBeVisible();
    await tabInto(() => canvas.getByRole("grid", { name: "Each program's own skills" }));
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(canvas.getByRole("heading", { name: /^my-notes/ })).toBeInTheDocument(),
    );
    await tabInto(() => canvas.getByRole("switch", { name: "On for smart-ide's chats" }));
    expectFocusInSight();
  },
};

/** **Harness skills, narrow**: page-and-back, a skill drawn alone with its way back. */
export const HarnessSkillsNarrow: Story = {
  args: { width: 460, narrow: true },
  play: async ({ canvas }) => {
    await canvas.findByRole("grid", { name: "Each program's own skills" });
    await tabInto(() => canvas.getByRole("grid", { name: "Each program's own skills" }));
    await userEvent.keyboard("{ArrowDown}{Enter}");
    const back = await waitFor(() =>
      canvas.getByRole("button", { name: "Back to Harness skills" }),
    );
    await expect(back).toBeInTheDocument();
    await expect(canvas.getByText("Read-only", { selector: "header *" })).toBeInTheDocument();
  },
};

/**
 * **Routing**: the router with Change…, the budget and its grade, then each chat's start, the one
 * chosen beside the list with what it hid.
 */
export const Routing: Story = {
  args: { page: "routing" },
  play: async ({ canvas }) => {
    const grid = await canvas.findByRole("grid", { name: "Each chat's start" });
    await expect(within(grid).getByText("Fell back to the built-in router")).toBeVisible();
    await tabInto(() => canvas.getByRole("grid", { name: "Each chat's start" }));
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(canvas.getByText("figma-handoff, gpu-profile, lint-rs")).toBeInTheDocument(),
    );
    expectFocusInSight();
  },
};

/**
 * **A router's grade's scorecard** (#3617): opened from the router's Scorecard, beside the list:
 * its end, its totals, then each skill's trigger tests in both arms with each chat.
 */
export const RouterScorecard: Story = {
  args: { page: "routing", item: routerGradeItem({ grade: SCORECARD.grade }) },
  play: async ({ canvas }) => {
    await expect(await canvas.findByTestId("router-grade-scorecard-about")).toHaveTextContent(
      "Graded 2026-10-27 09:12 UTC, on the library at 3f9a2c1d7e40",
    );
    await expect(canvas.getByRole("heading", { name: "lint-rs" })).toBeInTheDocument();
    await tabInto(() => canvas.getByTestId("router-grade-scorecard"));
    expectFocusInSight();
  },
};

/** **Routing where the built-in router routes every chat**: nothing asked, nothing listed. */
export const RoutingBuiltIn: Story = {
  args: {
    page: "routing",
    answer: studio([], {
      workspace: "smart-ide",
      workspaces: ["smart-ide"],
      router: {
        ...ROUTER,
        said: "Chats here are routed by the built-in router.",
        extension: null,
        written: null,
        grade: null,
        chats: [],
      },
    }),
  },
  play: async ({ canvas }) => {
    await expect(await canvas.findByTestId("skill-routing-no-chats")).toBeInTheDocument();
    await tabInto(() => canvas.getByRole("button", { name: "Change…" }));
    expectFocusInSight();
  },
};

/** **Change…**: the built-in router and each router extension installed here, what changes said
 *  before it does. */
export const ChangeRouter: Story = {
  args: { page: "routing" },
  play: async ({ canvas, canvasElement }) => {
    await userEvent.click(await canvas.findByRole("button", { name: "Change…" }));
    const body = within(canvasElement.ownerDocument.body);
    const dialog = await body.findByRole("dialog", { name: "Change the router" });
    await within(dialog).findByRole("radio", { name: "relevance" });
    await userEvent.click(within(dialog).getByRole("radio", { name: "Built-in" }));
    await expect(dialog).toHaveTextContent(
      "Chats in smart-ide started after this are routed by the built-in router.",
    );
  },
};
