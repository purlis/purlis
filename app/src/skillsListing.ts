// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Studio, StudioSkill } from "./bindings";

/**
 * **What the Skills view lists** (SK-55, #3266; decisions amendment 2026-10-26): this workspace's
 * skills, grouped by level, from the Skill studio's own answer (`skill_studio`), so the view and
 * the studio cannot disagree. Plain data, so the grouping, the one mark a row carries and what it
 * says on hover are tested without drawing anything.
 */

/** **One skill as the view lists it**: the studio's row, and where it comes from beside its level. */
export type ListedSkill = {
  /** Its key in the list: unique, as a copy of a name at another level is another row. */
  key: string;
  skill: StudioSkill;
  /** The persona that brings it into every workspace, for a persona's own skill. */
  persona?: string;
  /** The chat program it is the own skill of, as a person reads its name (`Claude Code`). */
  program?: string;
};

/** **A group of the list**, in its order: what needs the person, each level, each program's own. */
export type GroupId =
  | "needs-you"
  | "built-in"
  | "project"
  | "workspace"
  | "personas"
  | "repo"
  | "local"
  | `program:${string}`
  | "not-in-chat";

export type SkillGroup = {
  id: GroupId;
  title: string;
  /** The title is data (a program's name), drawn as it is written. */
  named: boolean;
  rows: ListedSkill[];
};

/** The groups of purlis's own words, in the order they are drawn; each program's own come after
 *  Local, and what a chat does not get last. */
const TITLES: Record<Exclude<GroupId, `program:${string}`>, string> = {
  "needs-you": "Needs you",
  "built-in": "Built-in",
  project: "Project",
  workspace: "Workspace",
  personas: "Personas",
  repo: "Repo",
  local: "Local",
  "not-in-chat": "Not in this chat",
};

/** **The one state a row is marked with**, where it has one (the amendment's (b)). */
export type SkillMark = "needs-review" | "off" | "left-out" | "lint";

/** What a mark says on its row. */
export const MARK_SAID: Record<SkillMark, string> = {
  "needs-review": "Needs review",
  off: "Off",
  "left-out": "Left out",
  lint: "Lint",
};

/**
 * **The one mark a row carries, by what matters most**: a folder waiting for the person's accept
 * (SK-9), a project skill this workspace turned off, a skill the chat is not given, then what the
 * built-in lints say. A row with none of these is plain.
 */
export function markOf(skill: StudioSkill): SkillMark | undefined {
  if (skill.accept !== null) return "needs-review";
  if (skill.turned_off) return "off";
  if (!skill.on) return "left-out";
  if (skill.lint.length > 0) return "lint";
  return undefined;
}

/** **Why a row has its mark**, in the core's own sentence where it has one. */
export function reasonOf(skill: StudioSkill): string | undefined {
  switch (markOf(skill)) {
    case "needs-review":
    case "off":
    case "left-out":
      return skill.left_out ?? undefined;
    case "lint":
      return skill.lint.join(" ");
    case undefined:
      return undefined;
  }
}

/** **What a row says on hover**: what the skill does, then why it has its mark. */
export function tipOf(row: ListedSkill): string[] {
  const reason = reasonOf(row.skill);
  return [
    row.skill.description,
    ...(row.persona === undefined ? [] : [`${row.persona} brings it into every workspace.`]),
    ...(reason === undefined || reason === "" ? [] : [reason]),
  ].filter((line) => line.trim() !== "");
}

/** The level group a row is listed under. */
function levelGroup(row: ListedSkill): GroupId {
  const { skill } = row;
  if (skill.local && (skill.level === "project" || skill.level === "workspace")) return "local";
  switch (skill.level) {
    case "built-in":
      return "built-in";
    case "project":
      return "project";
    case "workspace":
      return "workspace";
    case "persona":
      return skill.local ? "local" : "personas";
    case "repo":
      return "repo";
    case "installed":
      return `program:${row.program ?? ""}`;
  }
}

/** **What the project's root lists** (the amendment's (g)): the project's and the personas'. */
const AT_THE_ROOT: readonly GroupId[] = ["needs-you", "project", "personas", "local"];

/**
 * **The list's groups, in their order, each with rows**: Needs you on top while it holds
 * anything, then Built-in, Project, Workspace, Personas, Repo, Local and each program's own. At
 * the project's root only the project's and the personas' are listed.
 *
 * **For one chat** (`chat`, the amendment's (c)), the rows are that chat's set: what it does not
 * get is kept, under one group at the end, each with its mark.
 */
export function groupsOf(
  rows: readonly ListedSkill[],
  at: { root: boolean; chat: boolean },
): SkillGroup[] {
  const by = new Map<GroupId, ListedSkill[]>();
  const add = (id: GroupId, row: ListedSkill) => by.set(id, [...(by.get(id) ?? []), row]);
  for (const row of rows) {
    const mark = markOf(row.skill);
    if (mark === "needs-review") add("needs-you", row);
    else if (at.chat && (mark === "left-out" || mark === "off")) add("not-in-chat", row);
    else add(levelGroup(row), row);
  }
  const programs = [...by.keys()].filter((id) => id.startsWith("program:"));
  const order: GroupId[] = [
    "needs-you",
    "built-in",
    "project",
    "workspace",
    "personas",
    "repo",
    "local",
    ...programs,
    "not-in-chat",
  ];
  return order
    .filter((id) => !at.root || AT_THE_ROOT.includes(id))
    .flatMap((id): SkillGroup[] => {
      const listed = by.get(id);
      if (listed === undefined || listed.length === 0) return [];
      const program = id.startsWith("program:");
      return [
        {
          id,
          title: program ? id.slice("program:".length) : TITLES[id as keyof typeof TITLES],
          named: program,
          rows: listed,
        },
      ];
    });
}

/** A row's key: its level, whether it is local, its folder, its name, and whose it is. */
function keyOf(skill: StudioSkill, persona?: string, program?: string): string {
  return [skill.level, skill.local, skill.folder, skill.name, persona ?? "", program ?? ""].join(
    "\u0000",
  );
}

/**
 * **The rows of one studio answer**: its plane's skills, then each program's own and each repo's,
 * a repo's skill once however many programs find it. Answered for a persona's chats, its own
 * skills are marked as that persona's.
 */
export function listedOf(studio: Studio, persona?: string): ListedSkill[] {
  const plane = studio.skills.map((skill): ListedSkill => {
    const whose = skill.level === "persona" ? persona : undefined;
    return whose === undefined
      ? { key: keyOf(skill), skill }
      : { key: keyOf(skill, whose), skill, persona: whose };
  });
  const seen = new Set<string>();
  const outside = studio.harnesses.flatMap((harness) =>
    harness.skills.flatMap((skill): ListedSkill[] => {
      const program = skill.level === "repo" ? undefined : harness.title;
      const key = keyOf(skill, undefined, program);
      if (seen.has(key)) return [];
      seen.add(key);
      return [{ key, skill, program }];
    }),
  );
  return [...plane, ...outside];
}

/** **The personas whose skills the project lists** (the studio's levels, SK-32), each once. */
export function personasOf(studio: Studio): string[] {
  return [
    ...new Set(
      studio.levels.flatMap((one) => (one.level.at === "persona" ? [one.level.name] : [])),
    ),
  ];
}

/** **A persona's own skills, from the studio answered for its chats**: each marked as its. */
export function personaRowsOf(persona: string, studio: Studio): ListedSkill[] {
  return listedOf(studio, persona).filter((row) => row.persona !== undefined);
}

/** Why a built-in skill has no switch: the studio's words and the Skills view's. */
export const BUILT_IN_STAYS = "It ships with purlis, and cannot be turned off.";

/**
 * **A project skill's switch in one workspace** (#2770): what it says and what it asks for, or
 * why there is none. The studio's row and the Skills view's menu both draw this.
 */
export type Turn = { label: string; on: boolean; refused: string } | { reason: string };

export function turnOf(skill: StudioSkill, workspace: string | undefined): Turn {
  if (skill.level === "built-in") return { reason: BUILT_IN_STAYS };
  if (skill.level !== "project")
    return { reason: "Only a project skill is turned on or off for one workspace." };
  if (workspace === undefined)
    return {
      reason: "A project skill is turned on or off for one workspace. Pick one in the title bar.",
    };
  if (skill.not_granted !== null) return { reason: skill.not_granted };
  return skill.on
    ? { label: `Turn off in ${workspace}`, on: false, refused: "The skill was not turned off" }
    : { label: `Turn on in ${workspace}`, on: true, refused: "The skill was not turned on" };
}

/** What a row's menu can do: the amendment's (b). Delete, move, rename, grants and publish stay
 *  in the skill's tab. */
export type SkillAct = "use" | "turn" | "explorer";

/** One row of a skill's menu: what it says, and why it cannot run where it cannot. */
export type SkillMenuRow = { act: SkillAct; title: string; available: boolean; reason: string };

/** **Show in Explorer's words**, and why it cannot run in a workspace: Explorer shows the
 *  project's own files only at its root. */
export const SHOW_IN_EXPLORER = "Show in Explorer";
const EXPLORER_AT_THE_ROOT =
  "Explorer shows this workspace's repos. The skill's folder is in the project's own files, which Explorer shows at the project's root.";
const NOT_IN_THE_PROJECT = "Its folder is not one of the project's files.";

/** **Whether a skill's folder is one of the project's own files**, which Explorer can show. */
export function inTheProject(skill: StudioSkill): boolean {
  return (
    (skill.level === "project" || skill.level === "workspace" || skill.level === "persona") &&
    skill.folder !== "" &&
    !skill.folder.startsWith("/")
  );
}

/**
 * **A row's menu, in its order**: Use in this chat while a chat is in front (`chat`), the
 * switch, and Show in Explorer. A row that cannot run says why.
 */
export function menuRowsOf(
  row: ListedSkill,
  at: { workspace: string | undefined; chat: boolean },
): SkillMenuRow[] {
  const { skill } = row;
  const turn = turnOf(skill, at.workspace);
  const explorer = !inTheProject(skill)
    ? NOT_IN_THE_PROJECT
    : at.workspace !== undefined
      ? EXPLORER_AT_THE_ROOT
      : "";
  return [
    ...(at.chat
      ? [{ act: "use" as const, title: "Use in this chat", available: true, reason: "" }]
      : []),
    "label" in turn
      ? { act: "turn", title: turn.label, available: true, reason: "" }
      : {
          act: "turn",
          title: skill.on ? "Turn off" : "Turn on",
          available: false,
          reason: turn.reason,
        },
    { act: "explorer", title: SHOW_IN_EXPLORER, available: explorer === "", reason: explorer },
  ];
}
