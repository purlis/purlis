// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { SkillGrant, SkillLevel, StudioLevel, StudioSkill } from "./bindings";
import type { StudioScope } from "./skillStudio";

/**
 * **The levels and grants the Skill studio's changes pick from** (SK-7, #2770). Which levels a
 * project has, and their words, are the core's (`Studio.levels`, `skills::verbs::Level`); this
 * only finds a row's level and compares two.
 */

/** The project's shared skills. */
export const PROJECT: SkillLevel = { workspace: null, local: false };

/** The level a row of the studio is at, or `null` for one the studio's changes do not move. A
 *  workspace's row is the workspace the studio shows. */
export function levelOf(skill: StudioSkill, workspace: string | null): SkillLevel | null {
  switch (skill.level) {
    case "project":
      return { workspace: null, local: skill.local };
    case "workspace":
      return workspace === null ? null : { workspace, local: skill.local };
    default:
      // A built-in skill is never changed, and a persona's skill (SK-5) is its own persona's.
      return null;
  }
}

/** **The level a new skill lands at: where the person came from** (decision 37): the workspace
 *  the studio shows, or the project's skills at its root. */
export function levelFrom(scope: StudioScope): SkillLevel {
  return scope.at === "workspace" ? { workspace: scope.name, local: false } : PROJECT;
}

/** Whether two levels are one. */
export function sameLevel(a: SkillLevel, b: SkillLevel): boolean {
  return a.workspace === b.workspace && a.local === b.local;
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
