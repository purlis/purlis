// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A skill's page inside the Skill studio** (#3682, `prototype/skill-page` variant A): the
 * header with its actions and "…" menu, the banner that says why a skill isn't plainly on with
 * its one fix, and the page's tabs with their counts. Drawn wide beside the Library and narrow
 * alone, for an ordinary skill, one that is off, a built-in one, and on its Text, Files and
 * History.
 * Walked by the keyboard, held to axe, and pictured in every pictured theme cell. The core is
 * faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, screen, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { language, syntaxTreeAvailable } from "@codemirror/language";
import { EditorView } from "@codemirror/view";
import { STORY_HISTORY as HISTORY, StudioInPanel } from "./skillStudioStory.testkit";
import { inWorkspace, skillRowId, type SkillPageTab } from "./skillStudio";
import { skill, studio } from "./skillsListing.testkit";
import { forgetViewHistory, openViewAt } from "./ui/panel/viewHistory";
import { forgetSkillEdit } from "./skillEdit";
import { expectFocusInSight, tabInto, untilFocused } from "./stories.testkit";
import type { StudioSkill } from "./bindings";

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
const BROWSER = skill("purlis-browser", {
  level: "built-in",
  label: "built-in",
  grant: null,
  granted: null,
  description: "Drives a real browser for a task that needs one, with credentials from a vault.",
  folder: "/app/plugin/skills/purlis-browser",
});
const SKILLS = [DEPLOY, LINT, BROWSER];
/** A skill installed from a source, whose folder purlis does not name: drawn only where chosen. */
const BRAINSTORM = skill("brainstorming", {
  folder: "",
  description: "Turns a rough idea into a design through questions, one at a time.",
});

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
  misses: 0,
};

const version = (commit: string, subject: string, at: string) => ({
  commit,
  at,
  author: "Ana",
  subject,
  runs: { ...RUNS, runs: 4, ended: 4, tool_calls: 9 },
  grade: null,
  before: null,
});

/** deploy's folder as the core walks it. */
const DEPLOY_FOLDER = {
  entries: [
    { path: "evals", kind: "folder", refused: null },
    { path: "evals/triggers.json", kind: "file", refused: null },
    { path: "references", kind: "folder", refused: null },
    { path: "references/rollback.md", kind: "file", refused: null },
    { path: "references/staging.md", kind: "file", refused: null },
    { path: "scripts", kind: "folder", refused: null },
    { path: "scripts/smoke.sh", kind: "file", refused: null },
    {
      path: "scripts/shared",
      kind: "link",
      refused: "'scripts/shared' is a link, and purlis opens no link in a skill's folder",
    },
    { path: "SKILL.md", kind: "file", refused: null },
  ],
  more: 0,
};

/** Each file's text, by its path in the folder. */
const FILE_TEXT: Record<string, string> = {
  "SKILL.md":
    "---\nname: deploy\n---\n\n# deploy\n\n1. Check the preconditions.\n2. Deploy in small steps.\n",
  "references/rollback.md":
    "# Rolling back\n\nTag the last good release, then run the rollback job for the service.\n",
  "references/staging.md": "# Staging\n\nStaging deploys need no approval.\n",
  "evals/triggers.json": '[{ "prompt": "ship the api", "loads": true }]\n',
  "scripts/smoke.sh": '#!/bin/sh\ncurl -fsS "$1/health"\n',
};

/** The studio tab, opened at `chosen`'s page on `tab`, `width` by `height` (560) px. */
function Tab({
  width,
  height = 560,
  narrow,
}: {
  width: number;
  height?: number;
  narrow?: boolean;
  chosen: StudioSkill;
  tab: SkillPageTab;
  /** Whether its text names skills bare, as hints (#5874). */
  hints?: boolean;
}) {
  return <StudioInPanel width={width} height={height} narrow={narrow} />;
}

const meta = {
  component: Tab,
  args: { width: 1100, chosen: DEPLOY, tab: "overview" },
  beforeEach: ({ args: args_ }) => {
    mockIPC((cmd, args) => {
      switch (cmd) {
        case "skill_studio":
          return studio(args_.chosen === BRAINSTORM ? [...SKILLS, BRAINSTORM] : SKILLS, {
            workspace: "smart-ide",
            workspaces: ["smart-ide"],
          });
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
            uses: args_.hints
              ? [
                  {
                    said: "skills:notify",
                    name: "notify",
                    project: true,
                    broken: false,
                    hint: false,
                    line: null,
                  },
                  {
                    said: '"grilling"',
                    name: "grilling",
                    project: true,
                    broken: false,
                    hint: true,
                    line: 14,
                  },
                ]
              : [],
            used_by: args_.hints ? ["landing-a-pr"] : [],
            hinted_by: args_.hints ? ["report-intake"] : [],
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
        case "skill_files":
          switch ((args as { folder?: string }).folder) {
            case BROWSER.folder:
              // A walk cut short: the line under the tree says how many more there are.
              return { entries: [{ path: "SKILL.md", kind: "file", refused: null }], more: 7 };
            case LINT.folder:
              throw "'skills/lint-rs' is not the folder of a skill the studio lists here: the skill may have moved or gone. Open it again from the Library";
            default:
              return DEPLOY_FOLDER;
          }
        case "skill_file": {
          const { folder, path } = args as { folder: string; path: string };
          // Each skill's own files: a built-in skill's SKILL.md is its own, never deploy's.
          const text =
            folder === BROWSER.folder
              ? "---\nname: purlis-browser\n---\n\n# purlis-browser\n\nDrive a real browser, with credentials from a vault.\n"
              : FILE_TEXT[path];
          return { kind: "text", text: text ?? "" };
        }
        case "skill_deletes_cut_short":
          return { settled: [], unsettled: [] };
        default:
          return [];
      }
    });
    openViewAt(HISTORY, {
      page: "library",
      scope: SCOPE,
      skill: skillRowId(args_.chosen),
      tab: args_.tab,
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

/**
 * **A skill that names another bare** (#5874, `prototype/skill-hints` A): the hint's row carries
 * the Hint mark, why on its own line and Write skills:grilling, which Tab reaches; a skill that
 * names this one bare is marked too.
 */
export const OverviewHints: Story = {
  args: { hints: true },
  play: async ({ canvas }) => {
    const write = await canvas.findByRole("button", { name: "Write skills:grilling" });
    await expect(canvas.getByRole("list", { name: "Skills that name deploy" })).toHaveTextContent(
      "It names deploy bare. Its own tab offers the fix.",
    );
    for (let step = 0; step < 60 && document.activeElement !== write; step++) await userEvent.tab();
    await untilFocused(() => write, "Tab reaches Write skills:grilling");
    expectFocusInSight();
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

/**
 * **Scrolled under its header** (#3944): the page's body, its tabs first, scrolls under the
 * header, which is drawn on its panel's fill, so none of it shows through behind the actions.
 * The keyboard on the page tabs brings them out from under the header, never leaves them behind
 * it (WCAG 2.4.11). Pictured scrolled to its end.
 */
async function scrolledUnderTheHeader(canvasElement: HTMLElement): Promise<void> {
  const canvas = within(canvasElement);
  const tabs = await canvas.findByRole("tablist", { name: "deploy's sections" });
  await canvas.findByTestId("skill-owners");
  const pane = tabs.closest<HTMLElement>('[data-scope="view-frame"][data-part="item"]');
  const header = pane?.querySelector<HTMLElement>('[data-scope="view-frame"][data-part="header"]');
  const panel = pane?.closest<HTMLElement>('[data-scope="panel"][data-part="root"]');
  if (!pane || !header || !panel) throw new Error("the skill's page is not in a pane of a panel");
  await expect(pane.scrollHeight).toBeGreaterThan(pane.clientHeight);
  // The header's ground is its panel's fill, so nothing shows through it.
  await expect(getComputedStyle(header).backgroundColor).toBe(
    getComputedStyle(panel).backgroundColor,
  );
  const toTheEnd = async () => {
    pane.scrollTop = pane.scrollHeight;
    await waitFor(() =>
      expect(tabs.getBoundingClientRect().bottom).toBeLessThanOrEqual(
        header.getBoundingClientRect().bottom,
      ),
    );
  };
  await toTheEnd();
  // The keyboard reaches the tabs from under the header: they are scrolled clear of it.
  const chosen = within(tabs).getByRole("tab", { name: /Overview/ });
  chosen.focus();
  expectFocusInSight();
  await expect(tabs.getBoundingClientRect().top).toBeGreaterThanOrEqual(
    header.getBoundingClientRect().bottom - 1,
  );
  // Pictured scrolled to its end, the keyboard gone from the page.
  chosen.blur();
  await toTheEnd();
}

/** **Scrolled**, wide beside the Library: the body under the header, clear of its actions. */
export const Scrolled: Story = {
  args: { height: 360 },
  play: async ({ canvasElement }) => scrolledUnderTheHeader(canvasElement),
};

/** **Scrolled, narrow**: the page alone, its header wrapping, the body under it all the same. */
export const ScrolledNarrow: Story = {
  args: { width: 460, height: 360, narrow: true },
  play: async ({ canvasElement }) => scrolledUnderTheHeader(canvasElement),
};

/** Edit, then a line typed at the end of the text: the editor in place, as the person types. */
async function typeALine(canvas: ReturnType<typeof within>): Promise<void> {
  await userEvent.click(await canvas.findByRole("button", { name: "Edit deploy" }));
  // The editor is a chunk of its own, loaded the first time: given the time a busy cell takes.
  await canvas.findByRole("textbox", { name: "deploy's instructions" }, { timeout: 10_000 });
  // Its grammar is a chunk of its own as well, which lands after the editor is up (`colour`) and
  // draws every line again. Landing between two keys, it drew the line being typed again under
  // user-event's caret, which WebKit then put at the line's start: "5." ended the line typed
  // (#4026). Typed into once the grammar is in and has read the whole text.
  // The text found again on each look, should the editor be made again meanwhile.
  const text = await waitFor(
    () => {
      const now = canvas.getByRole("textbox", { name: "deploy's instructions" });
      const view = EditorView.findFromDOM(now);
      expect(view, "the text is a CodeMirror view's").not.toBeNull();
      expect(view?.state.facet(language), "its grammar is in").toBeTruthy();
      expect(view !== null && syntaxTreeAvailable(view.state), "it has read the text").toBe(true);
      return now;
    },
    { timeout: 10_000 },
  );
  await userEvent.click(text);
  await userEvent.keyboard("{Control>}{End}{/Control}{Meta>}{ArrowDown}{/Meta}");
  // Pasted, in one input, not typed a key at a time: on the text's empty last line, WebKit's
  // editor could draw the line again between two keys and put the caret back at its start, so
  // the line came out as " Tag the release.5." in some runs (#4078).
  await userEvent.paste("5. Tag the release.");
  await expect(await canvas.findByTestId("skill-unsaved")).toHaveTextContent("Unsaved changes");
}

/**
 * **Editing in place** (#3683, S4): the Text tab is the editor, checked as the person types, with
 * Save and Discard, and the header says the changes are unsaved. Tab reaches Save.
 */
export const Editing: Story = {
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

/**
 * **Its Files**: the skill's folder as a tree beside the preview. Tab reaches the tree; → opens
 * a folder and moves into it, ↓ walks it, and Enter shows a file; the picture is a reference
 * rendered beside the tree.
 */

/** The Files tab's first draw loads the light editor's chunk and reads the folder: on a busy
 *  runner, more than the second a find waits by default. */
const FIRST_DRAW = { timeout: 10_000 };

export const Files: Story = {
  args: { tab: "files" },
  play: async ({ canvas }) => {
    const tree = await canvas.findByRole("treegrid", { name: "Files of deploy" }, FIRST_DRAW);
    const row = (name: string) => within(tree).getByRole("row", { name });
    await tabInto(() => tree);
    await untilFocused(() => row("evals"), "Files: Tab lands on the first row");
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => row("references"), "Files: Down moves to references");
    await userEvent.keyboard("{ArrowRight}");
    await waitFor(() => expect(row("references")).toHaveAttribute("aria-expanded", "true"));
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(() => row("rollback.md"), "Files: Right moves into the open folder");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(await canvas.findByRole("heading", { name: "Rolling back" })).toBeInTheDocument();
    await untilFocused(() => row("rollback.md"), "Files: the keyboard stays on the file shown");
  },
};

/** **A built-in skill's Files**: shown, and said to be read only, with nothing that changes it;
 *  the walk was cut short, and the line under the tree says so. */
export const FilesBuiltIn: Story = {
  args: { chosen: BROWSER, tab: "files" },
  play: async ({ canvas }) => {
    const tree = await canvas.findByRole(
      "treegrid",
      { name: "Files of purlis-browser" },
      FIRST_DRAW,
    );
    await tabInto(() => tree);
    await userEvent.keyboard("{Enter}");
    await expect(await canvas.findByTestId("skill-files-read-only")).toHaveTextContent(
      "Read only: built-in skills can't be changed.",
    );
    await expect(canvas.queryByRole("button", { name: "Edit SKILL.md" })).toBeNull();
    await expect(canvas.getByTestId("skill-files-more")).toHaveTextContent("At least 7 more");
    expectFocusInSight();
  },
};

/** **Its Files before a file is picked**: the preview says how to pick one, and a link in the
 *  folder is a row that says why it does not open, on the keyboard too. */
export const FilesUnpicked: Story = {
  args: { tab: "files" },
  play: async ({ canvas }) => {
    const tree = await canvas.findByRole("treegrid", { name: "Files of deploy" }, FIRST_DRAW);
    const row = (name: string) => within(tree).getByRole("row", { name });
    await tabInto(() => tree);
    await userEvent.keyboard("{End}");
    await untilFocused(() => row("SKILL.md"), "Files: End moves to the last row");
    await userEvent.keyboard("{Home}{ArrowDown}{ArrowDown}{ArrowRight}");
    await waitFor(() => expect(row("scripts")).toHaveAttribute("aria-expanded", "true"));
    await userEvent.keyboard("{ArrowDown}{ArrowDown}");
    await untilFocused(() => row("shared"), "Files: Down reaches the link");
    await expect(row("shared")).toHaveAttribute("aria-disabled", "true");
    await userEvent.keyboard("{Enter}");
    await expect(canvas.getByText("Pick a file to read it")).toBeVisible();
    expectFocusInSight();
  },
};

/** **A skill with no folder purlis names** (installed from a source): its Files says why. */
export const FilesNone: Story = {
  args: { chosen: BRAINSTORM, tab: "files" },
  play: async ({ canvas }) => {
    await expect(await canvas.findByTestId("skill-files-none", {}, FIRST_DRAW)).toHaveTextContent(
      "its files are kept with the project's pins",
    );
  },
};

/** **A folder that could not be read**: why, in place, with Try again on the keyboard. */
export const FilesNotRead: Story = {
  args: { chosen: LINT, tab: "files" },
  play: async ({ canvas }) => {
    await expect(
      await canvas.findByTestId("skill-page-not-read", {}, FIRST_DRAW),
    ).toHaveTextContent("Open it again from the Library");
    const again = canvas.getByRole("button", { name: "Try again" });
    await tabInto(() => again);
    expectFocusInSight();
  },
};
