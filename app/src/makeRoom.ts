// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Make room** (#5355; ruling of the operator, 2026-11-05): when a chat is over its skill
 * budget, Routing and the Install all review offer the Library's Select mode (#5332) opened with
 * the skills least used in the last 30 days first, from the run records (`skill_uses`), and
 * nothing ticked: the person chooses. Only {@link MAKE_ROOM_ACTS} are offered there.
 */
import type { StudioBudget, StudioSkill } from "./bindings";
import type { SelectAct } from "./SkillLibrarySelect";
import { counted } from "./dispatches";

/** **What Make room offers** for the skills ticked: turn off or archive what goes, pin what
 *  stays. */
export const MAKE_ROOM_ACTS: readonly SelectAct[] = ["turn-off", "archive", "pin"];

/** What the Library's head says while Make room is on. */
export const MAKE_ROOM_LEAD =
  "Least used in the last 30 days first, from the chats' run records. Nothing is ticked until you tick it. Turn off or archive what goes, or pin what stays.";

/** "No runs in the last 30 days", "1 run in the last 30 days". */
export function usesSaid(runs: number | undefined): string {
  return runs === undefined || runs === 0
    ? "No runs in the last 30 days"
    : `${counted(runs, "run", "runs")} in the last 30 days`;
}

/** **`rows` with the least used first**, by `uses` of each one's name; equals keep their order. */
export function leastUsedFirst<T>(
  rows: readonly T[],
  nameOf: (row: T) => string,
  uses: ReadonlyMap<string, number>,
): T[] {
  return rows
    .map((row, at) => ({ row, at, runs: uses.get(nameOf(row)) ?? 0 }))
    .sort((a, b) => a.runs - b.runs || a.at - b.at)
    .map((one) => one.row);
}

/** **How many skills the router hides from these chats' lists**, their list being over the
 *  budget: what Routing's Make room is offered on. */
export function hiddenCount(skills: readonly StudioSkill[]): number {
  // An extension's router hides for reasons of its own: only a hide over the budget counts.
  return skills.filter((skill) => skill.hidden.some((on) => on.hidden.why === "over-budget"))
    .length;
}

/** What Routing says while these chats are over their budget. */
export function overSaid(hidden: number): string {
  return `The router hides ${counted(hidden, "skill", "skills")} from these chats' lists: they are over the budget.`;
}

/** What a program's chats' list counts now, as the core works it out (`skill_budget_preview`). */
export type ListCount = { listed: number; hidden: number };

/**
 * **Which programs an install of `adding` skills would take past their budget**, a sentence each:
 * what a chat's list counts now, listed and hidden, and the skills installed, against the most
 * skills it lists. A program already past it is past it still.
 */
export function installGoesOver(
  programs: readonly { program: StudioBudget; counted: ListCount }[],
  adding: number,
): string[] {
  return programs.flatMap(({ program, counted: now }) => {
    const then = now.listed + now.hidden + adding;
    return then > program.skills
      ? [
          `Installing these puts ${counted(then, "skill", "skills")} in a chat's list on ${program.program}, past its budget of ${program.skills}.`,
        ]
      : [];
  });
}
