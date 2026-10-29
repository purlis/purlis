// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Skill studio's frame and Library** (#3681, `prototype/skill-studio-frame` variant A):
 * one tab per project, its pages in a left navigation, its back, forward and path over them, the
 * scope picker, and the Library as a list (a name, its skill level, one mark and one line under
 * them) with a search box and filter chips. Drawn wide (three panes) and narrow (page-and-back),
 * while its skills are read, where they could not be, and on Needs you. Walked by the keyboard,
 * held to axe, and pictured in every pictured theme cell. The core is faked at its command
 * boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SkillStudioTab } from "./SkillStudioTab";
import {
  AT_THE_ROOT,
  SKILL_STUDIO_VIEW,
  inWorkspace,
  studioHistoryOf,
  type StudioPage,
  type StudioScope,
} from "./skillStudio";
import { need, skill, studio } from "./skillsListing.testkit";
import { forgetViewHistory, openViewAt } from "./ui/panel/viewHistory";
import { expectFocusInSight, untilFocused, untilFocusWithin } from "./stories.testkit";
import type { Studio } from "./bindings";

const PLANE = "/home/dev/plane";
const HISTORY = studioHistoryOf(PLANE, SKILL_STUDIO_VIEW);

/** The prototype's library, cut short: the project's own, a workspace's, and built-in ones. */
const SKILLS = [
  skill("deploy", {
    description:
      "Deploys a service to staging or production with the checks the team agreed on: tests green, a changelog entry and a tag.",
  }),
  skill("review-pr", {
    description: "Reviews a pull request against REVIEW.md: correctness first, then the standards.",
    pinned: true,
  }),
  skill("lint-rs", {
    level: "workspace",
    label: "workspace smart-ide",
    on: false,
    turned_off: true,
    left_out: "turned off in smart-ide",
    description: "Runs clippy with the workspace's flags and explains each warning in one line.",
    folder: "workspaces/smart-ide/skills/lint-rs",
  }),
  skill("figma-handoff", {
    on: false,
    left_out: "a built-in skill has this name, and a built-in skill is never replaced",
    description: "Checks a Figma frame against the built page and lists every gap.",
  }),
  skill("browser", {
    level: "built-in",
    label: "built-in",
    grant: null,
    granted: null,
    description: "Drives a real browser for a task that needs one, with credentials from a vault.",
    folder: "/app/plugin/skills/browser",
  }),
  skill("secrets", {
    level: "built-in",
    label: "built-in",
    grant: null,
    granted: null,
    description: "Uses a credential held in a vault without its value entering the conversation.",
    folder: "/app/plugin/skills/secrets",
  }),
];

/** What the faked core answers: the studio, or a refusal, or nothing at all yet. */
type Answer = Studio | { refused: string } | "reading";

/** The studio tab as a view tab draws it, `width` by 520 px, opened at `page` on `scope`. */
function Tab({
  width,
  narrow,
}: {
  width: number;
  narrow?: boolean;
  /** Where an entry point opened it: the page, and whose chats. */
  page: StudioPage;
  scope: StudioScope;
  answer: Answer;
}) {
  return (
    <div className="view-body" style={{ inlineSize: width, blockSize: 520, display: "flex" }}>
      <SkillStudioTab plane={PLANE} changed={0} narrow={narrow} />
    </div>
  );
}

const meta = {
  component: Tab,
  args: {
    width: 1000,
    page: "library",
    scope: inWorkspace("smart-ide"),
    answer: studio(SKILLS, { workspace: "smart-ide", workspaces: ["smart-ide", "git-ci-cd"] }),
  },
  beforeEach: ({ args }) => {
    const answer = args.answer;
    mockIPC((cmd) => {
      if (cmd === "skill_studio") {
        if (answer === "reading") return new Promise(() => undefined);
        if ("refused" in answer) throw answer.refused;
        return answer;
      }
      if (cmd === "skill_updates") return { offers: [], unconfirmed: [], unasked: [] };
      if (cmd === "skill_deletes_cut_short") return { settled: [], unsettled: [] };
      return [];
    });
    openViewAt(HISTORY, { page: args.page, scope: args.scope });
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
 * **Wide: three panes.** The navigation, the Library's rows (each one line under its name, cut
 * cleanly) and the skill chosen beside them. Tab reaches the list; ↓ and Enter choose a skill,
 * which opens beside it and joins the path; the navigation's one stop chooses another page.
 */
export const Wide: Story = {
  play: async ({ canvas }) => {
    const grid = await canvas.findByRole("grid", { name: "Skills" });
    await expect(canvas.getByRole("button", { name: "Library" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    // Nothing wraps into a tall row: every row is the same height, its line under its name.
    const heights = within(grid)
      .getAllByRole("row")
      .map((row) => Math.round(row.getBoundingClientRect().height));
    await expect(new Set(heights).size).toBe(1);

    await tabInto(() => canvas.getByRole("grid", { name: "Skills" }));
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(canvas.getByRole("heading", { name: /^deploy/ })).toBeInTheDocument(),
    );
    await expect(
      within(canvas.getByRole("list", { name: "Path" })).getAllByRole("listitem"),
    ).toHaveLength(2);
    await userEvent.keyboard("{ArrowDown}{ArrowDown}{Enter}");
    await waitFor(() =>
      expect(canvas.getByRole("heading", { name: /^lint-rs/ })).toBeInTheDocument(),
    );
    expectFocusInSight();
  },
};

/** **Filtered**: what is typed in the search box, and a chip ticked, narrow the list. */
export const Filtered: Story = {
  play: async ({ canvas }) => {
    await canvas.findByRole("grid", { name: "Skills" });
    const search = canvas.getByRole("searchbox", { name: "Search skills" });
    await untilFocused(() => {
      search.focus();
      return search;
    }, "the search box");
    await userEvent.keyboard("pull request");
    await waitFor(() =>
      expect(within(canvas.getByRole("grid", { name: "Skills" })).getAllByRole("row")).toHaveLength(
        1,
      ),
    );
    await userEvent.clear(search);
    await userEvent.tab();
    // The chips are one Tab stop; Space ticks the one the keyboard is on.
    await untilFocusWithin(() => canvas.getByRole("group", { name: "Show only" }), "the chips");
    await userEvent.keyboard("{ArrowRight}{ }");
    await waitFor(() =>
      expect(within(canvas.getByRole("grid", { name: "Skills" })).getAllByRole("row")).toHaveLength(
        2,
      ),
    );
    expectFocusInSight();
  },
};

/** **Narrow: page-and-back.** The navigation is a picker; a skill opened from the list is drawn
 *  alone with a way back, and back lands on the list. */
export const Narrow: Story = {
  args: { width: 460, narrow: true },
  play: async ({ canvas, canvasElement }) => {
    await canvas.findByRole("grid", { name: "Skills" });
    await expect(canvas.queryByRole("navigation", { name: "Skill studio pages" })).toBeNull();
    const root = canvasElement.querySelector<HTMLElement>('[data-part="root"][data-narrow]');
    if (root === null) throw new Error("the frame is not narrow");
    await expect(root.scrollWidth).toBeLessThanOrEqual(root.clientWidth + 1);
    await tabInto(() => canvas.getByRole("grid", { name: "Skills" }));
    await userEvent.keyboard("{Enter}");
    const back = await waitFor(() => canvas.getByRole("button", { name: "Back to Library" }));
    await untilFocused(() => back, "the way back");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await untilFocusWithin(() => canvas.getByRole("region", { name: "Library" }), "the list again");
  },
};

/** **While its skills are read**: the frame keeps its layout, with rows held in their place. */
export const Reading: Story = {
  args: { answer: "reading" },
  play: async ({ canvas }) => {
    await expect(await canvas.findByTestId("skill-studio-reading")).toHaveAttribute(
      "aria-busy",
      "true",
    );
    // After about a second a line says what is happening; the picture is taken with it there.
    await canvas.findByText("Reading the project's skills…", undefined, { timeout: 3_000 });
    await tabInto(() => canvas.getByRole("navigation", { name: "Skill studio pages" }));
    expectFocusInSight();
  },
};

/** **Where they could not be read**: what failed, in place, with Try again. */
export const NotRead: Story = {
  args: { answer: { refused: "purlis could not open skills/: permission denied" } },
  play: async ({ canvas }) => {
    const refused = await canvas.findByTestId("skill-studio-refused");
    await expect(refused).toHaveTextContent("permission denied");
    const again = within(refused).getByRole("button", { name: "Try again" });
    await tabInto(() => again);
    expectFocusInSight();
  },
};

/** **Needs you** (#3689): what needs a decision, grouped by kind, the first item beside the list
 *  in the attention tone with its one fix. */
export const NeedsYou: Story = {
  args: {
    page: "needs",
    scope: AT_THE_ROOT,
    answer: studio(SKILLS, {
      needs: [
        need("grilling", { tag: "persona steward" }),
        need("clash:Project:deploy", {
          kind: "clash",
          title: "deploy",
          reason: "The repo svc has a skill of this name too, and a chat there gets both.",
          fix: { kind: "rename", name: "deploy", level: "project", local: false },
          fix_label: "Rename it…",
          fix_does: "Opens its page with Rename, so only one copy keeps the name.",
        }),
        need("tidy-up:unmoved", {
          kind: "tidy-up",
          title: "2 skills only some programs read",
          tag: null,
          reason:
            "This project keeps 2 skills where only some chat programs read them. Moving them gives every chat each one, whichever program it runs on.",
          fix: { kind: "move-skills", fix: "project-skills" },
          fix_label: "Move them",
          fix_does: "One commit, which can be undone.",
        }),
      ],
    }),
  },
  play: async ({ canvas }) => {
    const needs = await canvas.findByRole("button", { name: /^Needs you/ });
    await waitFor(() => expect(needs).toHaveAttribute("aria-current", "page"));
    await expect(canvas.getByTestId("skill-need")).toHaveAttribute("data-need", "grilling");
    await tabInto(() => canvas.getByRole("button", { name: "Accept it" }));
    expectFocusInSight();
  },
};
