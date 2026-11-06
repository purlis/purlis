// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Make room** (#5355): the Routing page offers it while these chats are over their skill
 * budget, and it opens the Library's Select mode (#5332) with the skills least used in the last
 * 30 days first, each saying its runs, nothing ticked, and only Turn off, Archive and Pin.
 * Walked by the keyboard, held to axe, and pictured in every pictured theme cell. The core is
 * faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { SkillHiddenOn, StudioRouter } from "./bindings";
import { STORY_HISTORY as HISTORY, StudioInPanel } from "./skillStudioStory.testkit";
import { inWorkspace } from "./skillStudio";
import { skill, studio } from "./skillsListing.testkit";
import { expectFocusInSight, tabInto } from "./stories.testkit";
import { forgetViewHistory, openViewAt } from "./ui/panel/viewHistory";

const HIDDEN: SkillHiddenOn[] = [
  {
    harness: "claude",
    title: "Claude Code",
    hidden: {
      router: "built-in",
      why: "over-budget",
      said: "Hidden on Claude Code: past the budget.",
    },
  },
];

const SKILLS = [
  skill("deploy", { description: "Deploys a service to staging or production." }),
  skill("grilling", { description: "Grills the person about a plan until the gaps show." }),
  skill("tdd", { description: "Test-driven development: red, green, refactor." }),
  skill("release-notes", {
    description: "Writes the release notes from the merged pull requests.",
    hidden: HIDDEN,
  }),
  skill("research", {
    description: "Investigates a question against primary sources.",
    hidden: HIDDEN,
  }),
];

const USES = [
  { skill: "deploy", runs: 31 },
  { skill: "tdd", runs: 12 },
  { skill: "grilling", runs: 1 },
];

const ROUTER: StudioRouter = {
  said: "Chats here are routed by the built-in router.",
  problem: false,
  extension: null,
  chats: [],
  grade: null,
  graded: null,
  written: null,
  budget: [
    {
      program: "Claude Code",
      key: "claude",
      skills: 3,
      chars: 15000,
      skills_said: "3, this workspace · Claude Code's default is 60",
      chars_said: "15,000, Claude Code's default",
      skills_here: 3,
      chars_here: null,
    },
  ],
  past_limits: [],
  preview:
    "The rows' hidden marks are what the built-in router hides for a chat at the workspace's root.",
};

/** The studio tab as a view tab draws it, `width` by 560 px. */
function Tab({ width }: { width: number }) {
  return <StudioInPanel width={width} height={560} />;
}

const meta = {
  component: Tab,
  args: { width: 1000 },
  beforeEach: () => {
    mockIPC((cmd) => {
      if (cmd === "skill_studio") return studio(SKILLS, { workspace: "smart-ide", router: ROUTER });
      if (cmd === "skill_uses") return USES;
      if (cmd === "skill_deletes_cut_short") return { settled: [], unsettled: [] };
      return [];
    });
    openViewAt(HISTORY, { page: "routing", scope: inWorkspace("smart-ide") });
    return () => {
      clearMocks();
      forgetViewHistory(HISTORY);
    };
  },
} satisfies Meta<typeof Tab>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **Offered on Routing**: how many skills the router hides, and Make room…, by the keyboard. */
export const Offered: Story = {
  play: async ({ canvas }) => {
    const over = await canvas.findByTestId("skill-routing-over");
    await expect(over).toHaveTextContent("The router hides 2 skills from these chats' lists");
    await tabInto(() => within(over).getByRole("button", { name: "Make room…" }));
    expectFocusInSight();
  },
};

/**
 * **Least used first**: Make room… opens the Library's Select with the skills with no runs in the
 * last 30 days at the top, each saying its runs, nothing ticked; Space ticks one, and the page
 * offers only Turn off, Pin and Archive.
 */
export const LeastUsedFirst: Story = {
  play: async ({ canvas }) => {
    const over = await canvas.findByTestId("skill-routing-over");
    await tabInto(() => within(over).getByRole("button", { name: "Make room…" }));
    await userEvent.keyboard("{Enter}");
    await canvas.findByTestId("select-none");
    await waitFor(() =>
      expect(
        within(canvas.getByTestId("skill-release-notes")).getByTestId("skill-uses"),
      ).toHaveTextContent("No runs in the last 30 days"),
    );
    await tabInto(() => canvas.getByRole("grid", { name: "Skills" }));
    await userEvent.keyboard(" ");
    const page = await canvas.findByTestId("select-page");
    await waitFor(() => expect(page).toHaveTextContent("1 selected"));
    await expect(
      within(within(page).getByTestId("select-acts"))
        .getAllByRole("row")
        .map((row) => row.getAttribute("data-testid")),
    ).toEqual(["select-act-turn-off", "select-act-pin", "select-act-archive"]);
    expectFocusInSight();
  },
};
