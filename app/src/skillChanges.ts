// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { SkillGrant, SkillLevel, SkillWhose, StudioLevel, StudioSkill } from "./bindings";
import type { StudioScope } from "./skillStudio";

/**
 * **The levels and grants the Skill studio's changes pick from** (SK-7, #2770). Which levels a
 * project has, and their words, are the core's (`Studio.levels`, `skills::verbs::Level`); this
 * only finds a row's level and compares two.
 */

/** The project's shared skills. */
export const PROJECT: SkillLevel = { at: "project", local: false };

/** The level a row of the studio is at, or `null` for one the studio's changes do not move. A
 *  workspace's row is the workspace the studio shows, and a persona's row the persona it shows
 *  (SK-32, #3003). */
export function levelOf(skill: StudioSkill, scope: StudioScope): SkillLevel | null {
  switch (skill.level) {
    case "project":
      return { at: "project", local: skill.local };
    case "workspace":
      return scope.at === "workspace"
        ? { at: "workspace", name: scope.name, local: skill.local }
        : null;
    case "persona":
      return scope.at === "persona"
        ? { at: "persona", name: scope.name, local: skill.local }
        : null;
    default:
      // A built-in, harness or repo skill is never changed here.
      return null;
  }
}

/** **The level a new skill lands at: where the person came from** (decision 37): the workspace
 *  or the persona the studio shows, or the project's skills at its root. A persona the project
 *  lists no level for (one made as one file keeps no skills) starts at the project's. */
export function levelFrom(scope: StudioScope, levels: readonly StudioLevel[]): SkillLevel {
  let from: SkillLevel;
  switch (scope.at) {
    case "root":
      return PROJECT;
    case "workspace":
      from = { at: "workspace", name: scope.name, local: false };
      break;
    case "persona":
      from = { at: "persona", name: scope.name, local: false };
      break;
  }
  const listed = from.at !== "persona" || levels.some((one) => sameLevel(one.level, from));
  return listed ? from : PROJECT;
}

/** **The levels the studio offers from `scope`** (SK-32): the project's, each workspace's, and
 *  the persona's it shows. Another persona's skills are changed from that persona's studio. */
export function levelsFor(levels: readonly StudioLevel[], scope: StudioScope): StudioLevel[] {
  return levels.filter(
    (one) =>
      one.level.at !== "persona" || (scope.at === "persona" && one.level.name === scope.name),
  );
}

/** Whether two levels are one. */
export function sameLevel(a: SkillLevel, b: SkillLevel): boolean {
  if (a.at !== b.at || a.local !== b.local) return false;
  if (a.at === "project" || b.at === "project") return true;
  return a.name === b.name;
}

/** A level's key among the studio's drop targets: `project`, `ops-local`, `persona-reviewer`. */
export function levelKey(level: SkillLevel): string {
  const local = level.local ? "-local" : "";
  switch (level.at) {
    case "project":
      return `project${local}`;
    case "workspace":
      return `${level.name}${local}`;
    case "persona":
      return `persona-${level.name}${local}`;
  }
}

/** Whose skills `level` is, without shared or local: what Share takes. */
export function whoseOf(level: SkillLevel): SkillWhose {
  switch (level.at) {
    case "project":
      return { at: "project" };
    case "workspace":
      return { at: "workspace", name: level.name };
    case "persona":
      return { at: "persona", name: level.name };
  }
}

/** A level's words, as the core gives them in `levels`. */
export function labelOf(levels: readonly StudioLevel[], level: SkillLevel): string {
  return levels.find((one) => sameLevel(one.level, level))?.label ?? "";
}

/** Whether two grants are one. */
export function sameGrant(a: SkillGrant, b: SkillGrant): boolean {
  if (a.to !== b.to) return false;
  if (a.to !== "only" || b.to !== "only") return true;
  return a.workspaces.join("\n") === b.workspaces.join("\n");
}

/** What a grant reaches, as the grant control says it. */
export function grantLabel(grant: SkillGrant): string {
  if (grant.to === "root") return "The project's root only";
  if (grant.to === "all") return "Every workspace";
  return `The root and ${grant.workspaces.join(", ")}`;
}

/** The grants the grant control offers, in order: the root, every workspace, each one
 *  workspace, and the one it holds now where it names several. */
export function grantChoices(workspaces: readonly string[], now: SkillGrant | null): SkillGrant[] {
  const choices: SkillGrant[] = [
    { to: "root" },
    { to: "all" },
    ...workspaces.map((workspace): SkillGrant => ({ to: "only", workspaces: [workspace] })),
  ];
  if (now !== null && !choices.some((one) => sameGrant(one, now))) choices.push(now);
  return choices;
}

/**
 * **A picker over typed choices**: the options a `Select` draws, valued by each choice's place in
 * `choices`, the place of `now` among them, and the choice a picked value names. No choice is
 * spelled as a string to be read back.
 */
export function picking<T>(
  choices: readonly T[],
  label: (choice: T) => string,
  same: (a: T, b: T) => boolean,
  now?: T,
): {
  options: { value: string; label: string }[];
  value: string;
  chosen: (value: string) => T | undefined;
} {
  const at = now === undefined ? -1 : choices.findIndex((one) => same(one, now));
  return {
    options: choices.map((choice, index) => ({ value: String(index), label: label(choice) })),
    value: at < 0 ? "" : String(at),
    chosen: (value) => choices[Number(value)],
  };
}
