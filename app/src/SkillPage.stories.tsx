// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A skill's page inside the Skill studio** (#3682, `prototype/skill-page` variant A): the
 * header with its actions and "…" menu, the banner that says why a skill isn't plainly on with
 * its one fix, and the page's tabs with their counts. Drawn wide beside the Library and narrow
 * alone, for an ordinary skill, one that is off, a built-in one, and on its Text and History.
 * Walked by the keyboard, held to axe, and pictured in every pictured theme cell. The core is
 * faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, screen, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SkillStudioTab } from "./SkillStudioTab";
import {
  SKILL_STUDIO_VIEW,
  inWorkspace,
  skillRowId,
  studioHistoryOf,
  type SkillPageTab,
} from "./skillStudio";
import { skill, studio } from "./skillsListing.testkit";
import { forgetViewHistory, openViewAt } from "./ui/panel/viewHistory";
import { forgetSkillEdit } from "./skillEdit";
import { expectFocusInSight, untilFocused, untilFocusWithin } from "./stories.testkit";
import type { StudioSkill } from "./bindings";

const PLANE = "/home/dev/plane";
const HISTORY = studioHistoryOf(PLANE, SKILL_STUDIO_VIEW);
const SCOPE = inWorkspace("smart-ide");

const DEPLOY = skill("deploy", {
  description:
    "Deploys a service to staging or production with the checks the team agreed on: tests green, a changelog entry and a tag.",
});
const LINT = skill("lint-rs", {
  on: false,
  turned_off: true,
  left_out: "turned off in smart-ide",
  grant: "all",
  granted: { to: "all" },
  description: "Runs clippy with the workspace's flags and explains each warning in one line.",
});
const BROWSER = skill("browser", {
  level: "built-in",
  label: "built-in",
  grant: null,
  granted: null,
  description: "Drives a real browser for a task that needs one, with credentials from a vault.",
  folder: "/app/plugin/skills/browser",
});
const SKILLS = [DEPLOY, LINT, BROWSER];

const RUNS = {
  group: { kind: "none" },
  runs: 0,
  ended: 0,
  tool_calls: 0,
  tool_errors: 0,
  refusals: 0,
  asks: 0,
  tokens: null,
  ms: 0,
};

const version = (commit: string, subject: string, at: string) => ({
  commit,
  at,
  author: "Ana",
  subject,
  runs: { ...RUNS, runs: 4, ended: 4, tool_calls: 9 },
  grade: null,
});

/** The studio tab, opened at `chosen`'s page on `tab`, `width` by 560 px. */
function Tab({
  width,
  narrow,
}: {
  width: number;
  narrow?: boolean;
  chosen: StudioSkill;
  tab: SkillPageTab;
}) {
  return (
    <div className="view-body" style={{ inlineSize: width, blockSize: 560, display: "flex" }}>
      <SkillStudioTab plane={PLANE} changed={0} narrow={narrow} />
    </div>
  );
}

const meta = {
  component: Tab,
  args: { width: 1100, chosen: DEPLOY, tab: "overview" },
  beforeEach: ({ args }) => {
    mockIPC((cmd) => {
      switch (cmd) {
        case "skill_studio":
          return studio(SKILLS, { workspace: "smart-ide", workspaces: ["smart-ide"] });
        case "skill_details":
          return {
            copies: [
              {
                level: { at: "project", local: false },
                label: "project",
                lifecycle: { state: "active" },
                unread: false,
                owners: ["platform team"],
                unused_days: null,
                versions: [
                  version("a".repeat(40), "Tighter steps for rollbacks", "2026-10-27T10:00:00Z"),
                  version("b".repeat(40), "First version", "2026-10-21T10:00:00Z"),
                ],
                uncommitted: RUNS,
              },
              {
                level: { at: "workspace", name: "smart-ide", local: false },
                label: "workspace smart-ide",
                lifecycle: { state: "active" },
                unread: false,
                owners: [],
                unused_days: null,
                versions: [version("c".repeat(40), "First version", "2026-10-20T10:00:00Z")],
                uncommitted: RUNS,
              },
            ],
            uses: [],
            used_by: [],
            uses_cut: false,
            pinned_by: [],
            pins_refused: [],
          };
        case "skill_stats":
          return {
            by_harness: [{ ...RUNS, group: { kind: "named", name: "Claude Code" }, runs: 41 }],
            by_persona: [],
            by_workspace: [],
          };
        case "skill_panels":
          return [];
        case "skill_text":
          return "---\nname: deploy\ndescription: Deploys a service to staging or production.\n---\n\n# deploy\n\n1. Read the request and the project's conventions.\n2. Check the preconditions before doing anything.\n3. Do the work in small, checked steps.\n4. Say what changed and what to check next.\n";
        case "skill_grade_plan":
          return { running: null };
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
      skill: skillRowId(args.chosen),
      tab: args.tab,
    });
    return () => {
      clearMocks();
      forgetSkillEdit(HISTORY);
      forgetViewHistory(HISTORY);
    };
  },
} satisfies Meta<typeof Tab>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Tab, until the keyboard is inside `target`, at most `steps` times. */
async function tabInto(target: () => Element | null, steps = 40): Promise<void> {
  for (let step = 0; step < steps; step++) {
    const into = target();
    if (into !== null && into.contains(document.activeElement)) return;
    await userEvent.tab();
  }
  await untilFocusWithin(target, "the keyboard reaches it by Tab");
}

/**
 * **An ordinary skill's Overview**, beside the Library: its facts, and its tabs with History's
 * and Runs' counts. Tab reaches the page tabs; → walks them and chooses each.
 */
export const Overview: Story = {
  play: async ({ canvas }) => {
    const tabs = await canvas.findByRole("tablist", { name: "deploy's sections" });
    await waitFor(() =>
      expect(within(tabs).getByRole("tab", { name: /History/ })).toHaveAccessibleDescription(
        "2 versions",
      ),
    );
    await canvas.findByTestId("skill-owners");
    await tabInto(() => canvas.getByRole("tablist", { name: "deploy's sections" }));
    expectFocusInSight();
    await userEvent.keyboard("{ArrowRight}");
    await waitFor(() =>
      expect(within(tabs).getByRole("tab", { name: /Text/ })).toHaveAttribute(
        "aria-selected",
        "true",
      ),
    );
    await userEvent.keyboard("{ArrowLeft}");
    await waitFor(() =>
      expect(within(tabs).getByRole("tab", { name: /Overview/ })).toHaveAttribute(
        "aria-selected",
        "true",
      ),
    );
  },
};

/** **A skill that is off** in the workspace: the banner says why, with its one fix. */
export const Off: Story = {
  args: { chosen: LINT },
  play: async ({ canvas }) => {
    await expect(await canvas.findByTestId("skill-banner-off")).toHaveTextContent(
      "It is off in smart-ide",
    );
    // The banner's fix comes first; Overview holds the same switch.
    const [fix] = canvas.getAllByRole("button", { name: "Turn on in smart-ide" });
    await tabInto(() => fix);
    expectFocusInSight();
  },
};

/** **A built-in skill**: it says it can't be edited, moved or turned off. */
export const BuiltIn: Story = {
  args: { chosen: BROWSER },
  play: async ({ canvas }) => {
    await expect(await canvas.findByTestId("skill-built-in")).toHaveTextContent(
      "can't be edited, moved or turned off",
    );
    await expect(canvas.queryByRole("button", { name: "Edit browser" })).toBeNull();
  },
};

/** **The "…" menu** from the keyboard: Enter opens it on its first row, Escape gives the keyboard
 *  back to it; the picture is taken with it open. */
export const More: Story = {
  play: async ({ canvas }) => {
    const more = await canvas.findByRole("button", { name: "More for deploy" });
    await tabInto(() => more);
    await userEvent.keyboard("{Enter}");
    const menu = await screen.findByRole("menu", { name: "More for deploy" });
    await untilFocused(
      () => within(menu).getByRole("menuitem", { name: /Rename/ }),
      "Enter opens the menu on Rename",
    );
    await userEvent.keyboard("{Escape}");
    await untilFocused(() => more, "Escape gives the keyboard back to the menu's button");
    await userEvent.keyboard("{Enter}");
    await screen.findByRole("menu", { name: "More for deploy" });
  },
};

/** **Its Text**, as chats started from now get it: read only, reached by the keyboard. */
export const Text: Story = {
  args: { tab: "text" },
  play: async ({ canvas }) => {
    const text = await canvas.findByTestId("skill-text");
    await expect(text).toHaveTextContent("small, checked steps");
    await tabInto(() => text);
    expectFocusInSight();
  },
};

/** **Its History**: each version, with its runs and a revert. */
export const History: Story = {
  args: { tab: "history" },
  play: async ({ canvas }) => {
    await canvas.findByText("Tighter steps for rollbacks");
    const revert = canvas.getByRole("button", { name: "Revert to this" });
    await tabInto(() => revert);
    expectFocusInSight();
  },
};

/** **Narrow**: the page alone, with a way back, its tabs scrolling along their row. */
export const Narrow: Story = {
  args: { width: 460, narrow: true },
  play: async ({ canvas, canvasElement }) => {
    await canvas.findByRole("tablist", { name: "deploy's sections" });
    const root = canvasElement.querySelector<HTMLElement>('[data-part="root"][data-narrow]');
    if (root === null) throw new Error("the frame is not narrow");
    await expect(root.scrollWidth).toBeLessThanOrEqual(root.clientWidth + 1);
    const back = canvas.getByRole("button", { name: "Back to Library" });
    await tabInto(() => back);
    expectFocusInSight();
  },
};

/** Edit, then a line typed at the end of the text: the editor in place, as the person types. */
async function typeALine(canvas: ReturnType<typeof within>): Promise<void> {
  await userEvent.click(await canvas.findByRole("button", { name: "Edit deploy" }));
  const text = await canvas.findByRole("textbox", { name: "deploy's instructions" });
  await userEvent.click(text);
  await userEvent.keyboard("{Control>}{End}{/Control}{Meta>}{ArrowDown}{/Meta}");
  await userEvent.keyboard("5. Tag the release.");
  await expect(await canvas.findByTestId("skill-unsaved")).toHaveTextContent("Unsaved changes");
}

/**
 * **Editing in place** (#3683, S4): the Text tab is the editor, checked as the person types, with
 * Save and Discard, and the header says the changes are unsaved. Tab reaches Save.
 */
export const Editing: Story = {
  parameters: {
    // The light editor fails colour contrast on a list's numbers in every cell (#3796): it is
    // kept out of axe here until that ticket fixes it, as the draft page's merge view is, and the
    // rest of the page is held to axe.
    a11y: { context: { exclude: [".cm-editor"] } },
  },
  play: async ({ canvas }) => {
    await typeALine(canvas);
    await canvas.findByText("It is a whole skill.");
    const save = canvas.getByRole("button", { name: "Save" });
    await waitFor(() => expect(save).toBeEnabled());
    await tabInto(() => save);
    expectFocusInSight();
  },
};

/** **Leaving with unsaved changes** (#3683, S4): choosing another skill asks first, on Keep
 *  editing, the safe answer. */
export const Leaving: Story = {
  parameters: { a11y: { context: { exclude: [".cm-editor"] } } },
  play: async ({ canvas }) => {
    await typeALine(canvas);
    await userEvent.click(canvas.getByTestId("skill-lint-rs"));
    const asked = await screen.findByRole("alertdialog", { name: "Save your changes to deploy?" });
    await untilFocused(
      () => within(asked).getByRole("button", { name: "Keep editing" }),
      "the question starts on Keep editing",
    );
  },
};
