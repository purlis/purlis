// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type {
  ChatSkill,
  HarnessKind,
  PersonaSkills,
  SkillHidden,
  Studio,
  StudioSkill,
} from "./bindings";

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
  /** That program, by the word the project calls it, for a program's own skill. */
  harness?: HarnessKind;
  /** Whether the chat in front has it, where its set was read (Only this chat's): `false` for
   *  one it does not, which is listed under Not in this chat. */
  inChat?: boolean;
  /** Why a router hid it from the chat in front, where that chat's set was read: `null` for one
   *  the chat lists. */
  hiddenInChat?: SkillHidden | null;
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
export type SkillMark = "needs-review" | "off" | "left-out" | "hidden" | "lint";

/** What a mark says on its row. */
export const MARK_SAID: Record<SkillMark, string> = {
  "needs-review": "Needs review",
  off: "Off",
  "left-out": "Left out",
  hidden: "Hidden by the router",
  lint: "Lint",
};

/** Why a skill the studio lists is not in the chat in front's set, where nothing else says. */
const NOT_GIVEN = "The chat in front started without it.";

/**
 * **The one mark a row carries, by what matters most**: a folder waiting for the person's accept
 * (SK-9), a project skill this workspace turned off, a skill the chat is not given, one a router
 * hid behind the index skill (SK-15), then what the built-in lints say. A row with none of these
 * is plain.
 */
export function markOf(row: ListedSkill): SkillMark | undefined {
  const { skill } = row;
  if (skill.accept !== null) return "needs-review";
  if (skill.turned_off) return "off";
  // What the chat in front's set says, where it was read, else what the studio says.
  if (!(row.inChat ?? skill.on)) return "left-out";
  if (row.hiddenInChat === undefined ? skill.hidden.length > 0 : row.hiddenInChat !== null)
    return "hidden";
  if (skill.lint.length > 0) return "lint";
  return undefined;
}

/** **Why a row has its mark**, in the core's own sentence where it has one. */
function reasonOf(row: ListedSkill): string | undefined {
  const { skill } = row;
  switch (markOf(row)) {
    case "needs-review":
    case "off":
      return skill.left_out ?? undefined;
    case "left-out":
      return skill.left_out ?? NOT_GIVEN;
    case "hidden":
      return hiddenSaid(row);
    case "lint":
      return skill.lint.join(" ");
    case undefined:
      return undefined;
  }
}

/**
 * **Why a router hid a skill**, in the core's own line: for the chat in front, as its set says;
 * else with the programs whose chats it is hidden from, `Hidden by the built-in router: over this
 * chat's skill budget, so it is in the index skill (on Claude Code).`
 */
function hiddenSaid(row: ListedSkill): string {
  const sentence = (said: string) => `${said.charAt(0).toUpperCase()}${said.slice(1)}`;
  if (row.hiddenInChat !== undefined && row.hiddenInChat !== null)
    return `${sentence(row.hiddenInChat.said)}.`;
  const { hidden } = row.skill;
  const said = [...new Set(hidden.map((one) => one.hidden.said))].join("; ");
  return `${sentence(said)} (on ${hidden.map((one) => one.title).join(", ")}).`;
}

/** **What a row says on hover**: what the skill does, then why it has its mark. */
export function tipOf(row: ListedSkill): string[] {
  const reason = reasonOf(row);
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
 * **For one chat** (`chat`, the amendment's (c)), the rows are that chat's set, wherever it
 * works: what it does not get is kept, under one group at the end, each with its mark.
 */
export function groupsOf(
  rows: readonly ListedSkill[],
  at: { root: boolean; chat: boolean },
): SkillGroup[] {
  const by = new Map<GroupId, ListedSkill[]>();
  const add = (id: GroupId, row: ListedSkill) => by.set(id, [...(by.get(id) ?? []), row]);
  for (const row of rows) {
    const mark = markOf(row);
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
    .filter((id) => at.chat || !at.root || AT_THE_ROOT.includes(id))
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

/** A row's key: whose it is, its level, whether it is local, and its folder or its name. */
function keyOf(skill: StudioSkill, persona?: string, program?: string): string {
  const whose = persona ?? program ?? "";
  return [skill.level, skill.local ? "local" : "shared", whose, skill.folder || skill.name].join(
    ":",
  );
}

/**
 * **The rows of one studio answer**: its plane's skills, then each program's own and each repo's,
 * a repo's skill once however many programs find it.
 */
export function listedOf(studio: Studio): ListedSkill[] {
  const plane = studio.skills.map((skill): ListedSkill => ({ key: keyOf(skill), skill }));
  const seen = new Set<string>();
  const outside = studio.harnesses.flatMap((harness) =>
    harness.skills.flatMap((skill): ListedSkill[] => {
      const program = skill.level === "repo" ? undefined : harness.title;
      const key = keyOf(skill, undefined, program);
      if (seen.has(key)) return [];
      seen.add(key);
      return [
        program === undefined ? { key, skill } : { key, skill, program, harness: harness.harness },
      ];
    }),
  );
  return [...plane, ...outside];
}

/** **Each persona's own skills** (`skill_persona_skills`, one read), each marked as its. */
export function personaRowsOf(personas: readonly PersonaSkills[]): ListedSkill[] {
  return personas.flatMap(({ persona, skills }) =>
    skills.map((skill) => ({ key: keyOf(skill, persona), skill, persona })),
  );
}

/** A skill of a chat's set the studio does not list, as a row: on, with what it does. */
function rowOfChat(skill: ChatSkill): StudioSkill {
  return {
    name: skill.name,
    level: skill.level,
    label: skill.label,
    local: false,
    grant: null,
    granted: null,
    turned_off: false,
    not_granted: null,
    on: true,
    left_out: null,
    accept: null,
    description: skill.description,
    folder: "",
    written: skill.name,
    named: false,
    unchecked: null,
    owner: null,
    switch: null,
    compat: [],
    lint: [],
    hidden: [],
    pinned: false,
    pinned_by: [],
  };
}

/** The levels of the plane's and the built-in skills: the ones a chat's set (`chat_skills`)
 *  speaks for. A program's own and a repo's are the studio's to say. */
const PLANE_LEVELS: readonly string[] = ["built-in", "project", "workspace", "persona"];

/**
 * **The rows narrowed to one chat** (Only this chat's): the plane's and the built-in skills by
 * the chat's real set (`chat_skills`, what Use skill… types from), each marked by whether the chat
 * has it, and another persona's own left out; the chat's own program's skills (`harness`) and
 * each repo's as the studio marks them, since the set holds neither; another program's not at
 * all. A skill of the set the rows do not hold is listed as the set says it.
 */
export function narrowedTo(
  rows: readonly ListedSkill[],
  set: readonly ChatSkill[],
  chat: { persona: string | null; harness: HarnessKind | undefined },
): ListedSkill[] {
  const given = (skill: { name: string; level: string }) =>
    set.find((one) => one.name === skill.name && one.level === skill.level);
  const has = (skill: { name: string; level: string }) => given(skill) !== undefined;
  const hiddenIn = (skill: { name: string; level: string }) => given(skill)?.hidden ?? null;
  const kept = rows.flatMap((row): ListedSkill[] => {
    const { level } = row.skill;
    if (level === "installed") return row.harness === chat.harness ? [row] : [];
    if (!PLANE_LEVELS.includes(level)) return [row];
    const inChat = has(row.skill);
    if (!inChat && row.persona !== undefined && row.persona !== chat.persona) return [];
    return [{ ...row, inChat, hiddenInChat: hiddenIn(row.skill) }];
  });
  const missing = set
    .filter(
      (one) =>
        PLANE_LEVELS.includes(one.level) &&
        !kept.some((row) => row.skill.name === one.name && row.skill.level === one.level),
    )
    .map((one): ListedSkill => {
      const skill = rowOfChat(one);
      return {
        key: keyOf(skill, undefined, "chat"),
        skill,
        inChat: true,
        hiddenInChat: one.hidden,
      };
    });
  return [...kept, ...missing];
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
export type SkillAct = "use" | "turn" | "pin" | "explorer";

/** One row of a skill's menu: what it says, and why it cannot run where it cannot. */
export type SkillMenuRow = { act: SkillAct; title: string; available: boolean; reason: string };

const EXPLORER_AT_THE_ROOT =
  "Explorer shows this workspace's repos. The skill's folder is in the project's own files, which Explorer shows at the project's root.";
const NOT_IN_THE_PROJECT = "Its folder is not one of the project's files.";
const NOT_THIS_CHATS = "The chat in front does not get it.";

/** The levels a skill can be pinned at (SK-15d), as the studio pins them: the project's. */
const PINNABLE: readonly string[] = ["project", "workspace", "persona"];

/**
 * **A skill's pin in one workspace** (SK-15d, #3232): what Pin to this workspace says and asks
 * for, or why there is none. By name, as the core pins, through the studio's own command
 * (`skill_pin`), so the view and the studio share one copy of the pin. A pin still written on a
 * skill the workspace's chats no longer get can always be undone.
 */
export type Pin = { label: string; on: boolean; refused: string } | { reason: string };

export function pinOf(skill: StudioSkill, workspace: string | undefined): Pin {
  if (skill.level === "built-in") return { reason: "It ships with purlis, and is always listed." };
  if (!PINNABLE.includes(skill.level))
    return { reason: "Its program lists it, not purlis's router." };
  if (workspace === undefined)
    return { reason: "A skill is pinned for one workspace's chats. Pick one in the title bar." };
  const pinned = skill.pinned_by.some((by) => by.at === "workspace" && by.name === workspace);
  if (pinned) return { label: UNPIN_HERE, on: false, refused: "The skill was not unpinned" };
  if (!skill.on) return { reason: "This workspace's chats do not get it." };
  return { label: PIN_HERE, on: true, refused: "The skill was not pinned" };
}

const PIN_HERE = "Pin to this workspace";
const UNPIN_HERE = "Unpin from this workspace";

/** **Whether a skill's folder is one of the project's own files**, which Explorer can show. */
function inTheProject(skill: StudioSkill): boolean {
  return (
    (skill.level === "project" || skill.level === "workspace" || skill.level === "persona") &&
    skill.folder !== "" &&
    !skill.folder.startsWith("/")
  );
}

/**
 * **A row's menu, in its order**: Use in this chat while a chat is in front (`chat`), the
 * switch, Pin to this workspace (Unpin while it pins it), and Show in Explorer. A row that cannot run says why: Use in this chat, on a skill
 * the chat does not get.
 */
export function menuRowsOf(
  row: ListedSkill,
  at: { workspace: string | undefined; chat: boolean },
): SkillMenuRow[] {
  const { skill } = row;
  const turn = turnOf(skill, at.workspace);
  const pin = pinOf(skill, at.workspace);
  const explorer = !inTheProject(skill)
    ? NOT_IN_THE_PROJECT
    : at.workspace !== undefined
      ? EXPLORER_AT_THE_ROOT
      : "";
  const mark = markOf(row);
  const usable = mark !== "left-out" && mark !== "off" && mark !== "needs-review";
  return [
    ...(at.chat
      ? [
          {
            act: "use" as const,
            title: "Use in this chat",
            available: usable,
            reason: usable ? "" : NOT_THIS_CHATS,
          },
        ]
      : []),
    "label" in turn
      ? { act: "turn", title: turn.label, available: true, reason: "" }
      : {
          act: "turn",
          title: skill.on ? "Turn off" : "Turn on",
          available: false,
          reason: turn.reason,
        },
    "label" in pin
      ? { act: "pin", title: pin.label, available: true, reason: "" }
      : { act: "pin", title: PIN_HERE, available: false, reason: pin.reason },
    {
      act: "explorer",
      title: "Show in the Explorer view",
      available: explorer === "",
      reason: explorer,
    },
  ];
}
