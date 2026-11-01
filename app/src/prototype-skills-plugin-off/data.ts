// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** PROTOTYPE, throwaway (#4477): every name and count is stand-in; the states are measured. */
import type { ListedSkill } from "../skillsListing";
import { skill } from "../skillsListing.testkit";

export type StateId = "on" | "disabled" | "marketplace" | "project-false" | "elsewhere" | "screened";
export type WritingId = "reason" | "reason-and-fix" | "one-line";

export type OffState = {
  id: StateId;
  /** The bar's name for it. */
  name: string;
  /** Way A: the reason alone. */
  reason: string;
  /** Way B and C: the reason and what to do. */
  withFix: string;
  /** Way C: the plugin's one line. */
  line: string;
  /** The command the row copies, where one fixes it. */
  command?: string;
  /** What each source really said, measured on Claude Code 2.1.316. */
  facts: [string, string][];
};

const RECORD = "superpowers@market, user scope";

export const STATES: OffState[] = [
  {
    id: "on",
    name: "On",
    reason: "",
    withFix: "",
    line: "",
    facts: [
      ["installed_plugins.json", RECORD],
      ["~/.claude/settings.json", "enabledPlugins: true"],
      ["The chat's init event", "superpowers:brainstorming and the rest"],
    ],
  },
  {
    id: "disabled",
    name: "Disabled",
    reason: "Claude Code has the superpowers plugin turned off.",
    withFix: "Claude Code has the superpowers plugin turned off. Turn it on, then restart this chat.",
    line: "Off in Claude Code",
    command: "claude plugin enable superpowers@market",
    facts: [
      ["installed_plugins.json", RECORD],
      ["~/.claude/settings.json", "enabledPlugins: false"],
      ["claude plugin list", "✘ disabled"],
      ["The chat's init event", "no plugin, no skills"],
    ],
  },
  {
    id: "marketplace",
    name: "Marketplace gone",
    reason: "Claude Code cannot load the superpowers plugin: its marketplace, market, is gone.",
    withFix:
      "Claude Code cannot load the superpowers plugin: its marketplace, market, is gone. Add the marketplace again, or uninstall the plugin.",
    line: "Claude Code cannot load it",
    command: "claude plugin uninstall superpowers@market",
    facts: [
      ["installed_plugins.json", RECORD],
      ["~/.claude/settings.json", "enabledPlugins: true, no such marketplace"],
      ["claude plugin list", "✘ failed to load: Marketplace market not found"],
      ["The chat's init event", "no plugin, no skills"],
    ],
  },
  {
    id: "project-false",
    name: "Project says false",
    reason: "This project's Claude Code settings turn the superpowers plugin off.",
    withFix:
      "This project's Claude Code settings turn the superpowers plugin off. Change enabledPlugins in .claude/settings.json, then restart this chat.",
    line: "Off in this project",
    facts: [
      ["installed_plugins.json", RECORD],
      ["~/.claude/settings.json", "enabledPlugins: true"],
      ["<chat folder>/.claude/settings.json", "enabledPlugins: false"],
      ["The chat's init event", "no plugin, no skills"],
    ],
  },
  {
    id: "elsewhere",
    name: "Installed for another folder",
    reason: "The superpowers plugin is installed for another folder, ~/work/site.",
    withFix:
      "The superpowers plugin is installed for another folder, ~/work/site. Install it for your user to have it here, then restart this chat.",
    line: "Installed for ~/work/site",
    command: "claude plugin install superpowers@market --scope user",
    facts: [
      ["installed_plugins.json", "superpowers@market, project scope, ~/work/site"],
      ["~/.claude/settings.json", "no entry"],
      ["claude plugin list, in this folder", "✘ disabled"],
      ["The chat's init event", "no plugin, no skills"],
    ],
  },
  {
    id: "screened",
    name: "Project says true, unread",
    reason: "Only this project's settings turn the superpowers plugin on, and this chat does not read them.",
    withFix:
      "Only this project's settings turn the superpowers plugin on, and this chat does not read them. The chat's tab says why.",
    line: "Project settings unread",
    facts: [
      ["installed_plugins.json", RECORD],
      ["~/.claude/settings.json", "enabledPlugins: false"],
      ["<chat folder>/.claude/settings.json", "enabledPlugins: true"],
      ["purlis starts the chat with", "--setting-sources user"],
      ["The chat's init event", "no plugin, no skills"],
    ],
  },
];

export const WRITINGS: { id: WritingId; name: string; says: string }[] = [
  { id: "reason", name: "A: the reason", says: "Each card says why. The fix is left to the person." },
  {
    id: "reason-and-fix",
    name: "B: reason and fix",
    says: "Each card says why and what to do, with Copy command where a command fixes it.",
  },
  {
    id: "one-line",
    name: "C: one line a plugin",
    says: "The plugin folds to one line. Unfolded, its skills are listed and the fix closes the run.",
  },
];

const SUPERPOWERS = [
  "brainstorming",
  "dispatching-parallel-agents",
  "executing-plans",
  "finishing-a-development-branch",
  "receiving-code-review",
  "requesting-code-review",
  "subagent-driven-development",
  "systematic-debugging",
  "test-driven-development",
  "using-git-worktrees",
  "verification-before-completion",
  "writing-plans",
  "writing-skills",
].map((name) => `superpowers:${name}`);

const program = (name: string, description: string): ListedSkill => ({
  key: name,
  program: "Claude Code",
  harness: "claude",
  skill: skill(name, { level: "installed", label: "installed", folder: "", description }),
});

/** The plugin that is always on, so the on case is on screen in every state. */
const ALWAYS_ON = [
  program("pdf-tools:fill-form", "Fills a PDF form from the values you give it."),
  program("pdf-tools:merge", "Merges PDFs into one."),
  program("langfuse", "Reads traces and prompts from Langfuse."),
];

const PLANE = ["pipeline-review", "runner-sizing"].map(
  (name): ListedSkill => ({
    key: name,
    inChat: true,
    hiddenInChat: null,
    skill: skill(name, { description: `The workspace's own ${name} guide.` }),
  }),
);

const BUILT_IN = ["secrets", "browser", "persona"].map(
  (name): ListedSkill => ({
    key: name,
    inChat: true,
    hiddenInChat: null,
    skill: skill(name, {
      level: "built-in",
      label: "built-in",
      folder: "/app/skills",
      description: `purlis's ${name} skill.`,
    }),
  }),
);

/** In the index skill: a different thing from a plugin that is off, and Use still works. */
const HIDDEN = ["changelog", "release-notes"].map(
  (name): ListedSkill => ({
    key: name,
    inChat: true,
    hiddenInChat: {
      router: "built-in",
      why: "over-budget",
      said: "hidden by the built-in router: over this chat's skill budget, so it is in the index skill",
    },
    skill: skill(name, { description: `The project's ${name} skill.` }),
  }),
);

export type Shown = {
  inChat: ListedSkill[];
  builtIns: ListedSkill[];
  /** The router's. */
  hidden: ListedSkill[];
  /** Off in the chat's program. */
  off: ListedSkill[];
  /** The header's lines, as `chatSetSaid` writes them. */
  said: string[];
};

export function shownIn(state: OffState, writing: WritingId): Shown {
  const on = state.id === "on";
  const powers = SUPERPOWERS.map((name) =>
    program(name, `The Superpowers skill ${name.split(":")[1] ?? name}.`),
  );
  const off = on
    ? []
    : powers.map(
        (row): ListedSkill => ({
          ...row,
          inChat: false,
          skill: {
            ...row.skill,
            on: false,
            left_out: writing === "reason" ? state.reason : state.withFix,
          },
        }),
      );
  return {
    inChat: [...PLANE, ...ALWAYS_ON, ...(on ? powers : [])],
    builtIns: BUILT_IN,
    hidden: HIDDEN,
    off,
    said: [
      "5 skills of 30 · 1,180 of 6,000 characters · 2 skills in the index skill",
      // The one number that moves: what the program's plugins really bring to this chat.
      `Claude Code's own, not counted: ${on ? "16 skills, 3,410" : "3 skills, 386"} characters`,
    ],
  };
}

/** What Use types, or nothing while it is not offered. */
export function typedBy(row: ListedSkill): string {
  if (row.hiddenInChat) return `Load the skill ${row.skill.name} with skill_load and follow it.`;
  if (row.skill.level === "installed") return `/${row.skill.name}`;
  if (row.skill.level === "built-in") return `/purlis:${row.skill.name}`;
  return `/skills:${row.skill.name}`;
}
