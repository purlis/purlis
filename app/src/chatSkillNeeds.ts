// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type {
  PlaneUpdated,
  SkillBudget,
  SkillPinner,
  SkillRouting,
  SkillsMissing,
} from "./bindings";
import { counted } from "./dispatches";
import { missingSays } from "./SkillsMissingNotice";
import {
  AT_THE_ROOT,
  asPersona,
  pinnerSaid,
  scopeOfPinner,
  SKILL_STUDIO_TITLE,
  skillStudioTitle,
  type StudioScope,
} from "./skillStudio";

/**
 * **What the chat in front needs of the person about its skills, as a whole** (spec #3679 S8,
 * #3694): the lines that crowded the top of Use skill… and the chat's skills notices, said once,
 * in the Skills view's Needs you, each with its one fix. A skill's own needs (a change to review,
 * a lint) are its card's; these are the chat's.
 *
 * - **Its budget**: its set near or past purlis's skill budget, fixed on the studio's Routing.
 * - **Its pins**: its workspace's and persona's pins past the most it reads together, fixed in
 *   the studio of one who pins.
 * - **Its router**: a router extension that fell back at its start, seen on Routing.
 * - **Started without**: skills its persona names that it started without, fixed in the studio.
 * - **Changed since it started**: what is new, gone and changed, which it gets on a restart.
 */
export type ChatNeed = {
  /** Its key among the chat's needs. */
  id: "budget" | "pins" | "routing" | "missing" | "changed";
  /** What the card is called. */
  title: string;
  /** What it says, in the core's words where they are the core's. */
  says: string;
  fix: ChatNeedFix;
};

/** **A chat need's one fix**: the studio at a page, or the chat's restart. */
export type ChatNeedFix =
  | { act: "studio"; label: string; scope: StudioScope; page: "library" | "routing" | "harness" }
  | { act: "restart"; label: string };

/** What the chat's view knows of its skills as a whole: what Use skill… was told, what its
 *  "started without" notice says, and what changed since it started. */
export type ChatSkillsSaid = {
  budget: SkillBudget | null;
  routing: SkillRouting | null;
  missing: SkillsMissing | null;
  changed: PlaneUpdated | null;
};

/** The view's own words for a chat whose skills changed since it started (S8). */
export const CHANGED_TITLE = "Changed since it started";

/** **What changed, by name** (#3694): new, gone and changed, each part only where it has one. */
export function changedSays(changed: PlaneUpdated): string {
  const parts = [
    changed.skills_added.length > 0 ? `new: ${changed.skills_added.join(", ")}` : undefined,
    changed.skills_gone.length > 0 ? `gone: ${changed.skills_gone.join(", ")}` : undefined,
    changed.skills_edited.length > 0 ? `changed: ${changed.skills_edited.join(", ")}` : undefined,
  ].filter((one) => one !== undefined);
  const what =
    parts.length === 0
      ? "This chat's skills changed since it started."
      : `This chat's skills changed since it started (${parts.join("; ")}).`;
  return `${what} It keeps the skills it started with until it restarts on the same conversation.`;
}

/** **The chat's needs, in the order the person meets them**: what is wrong with its set now,
 *  then what it lacks, then what changed. `scope` is where the view's studio opens. */
export function chatNeedsOf(said: ChatSkillsSaid, scope: StudioScope): ChatNeed[] {
  const needs: ChatNeed[] = [];
  const routing = {
    act: "studio" as const,
    label: "Open routing",
    scope,
    page: "routing" as const,
  };
  const budget = said.budget;
  const warns = budget === null ? undefined : warnsSaid(budget);
  if (budget !== null && warns !== undefined)
    needs.push({
      id: "budget",
      title: "Near the skill budget",
      says: warns,
      fix: budgetFix(budget, routing),
    });
  const together = budget === null ? undefined : pinsTogetherSaid(budget);
  // One fix a card (S8): the studio of the first that pins; the text names each.
  const pinner = budget?.pins_together?.held[0]?.by;
  if (together !== undefined && pinner !== undefined) {
    const at = scopeOfPinner(pinner);
    needs.push({
      id: "pins",
      title: "Its pins are not read",
      says: together,
      fix: { act: "studio", label: `Open ${skillStudioTitle(at)}`, scope: at, page: "library" },
    });
  }
  if (said.routing?.fell_back === true)
    needs.push({
      id: "routing",
      title: "Its router fell back",
      says: said.routing.said,
      fix: routing,
    });
  const missing = said.missing;
  if (missing !== null) {
    // A repo's own copy of a built-in skill is one the chat got, beside purlis's: not counted.
    const count =
      missing.missing.length +
      missing.no_built_in.length +
      missing.broken.length +
      missing.by_a_repo.length;
    needs.push({
      id: "missing",
      title:
        count === 0
          ? "A repo copies a built-in skill"
          : `Started without ${counted(count, "skill", "skills")}`,
      says: missingSays("This chat", missing),
      fix: {
        act: "studio",
        label: "Open in Skill studio",
        scope: missing.persona === null ? AT_THE_ROOT : asPersona(missing.persona),
        page: "library",
      },
    });
  }
  if (said.changed?.skills === true)
    needs.push({
      id: "changed",
      title: CHANGED_TITLE,
      says: changedSays(said.changed),
      fix: { act: "restart", label: "Restart" },
    });
  return needs;
}

/** **Where a budget past its most is fixed**, as its sentence says (`warnsSaid`): in the studio of
 *  whoever pins, for pins past it; on Harness skills, for a program's and repos' own past it; on
 *  Routing otherwise. */
function budgetFix(budget: SkillBudget, routing: ChatNeedFix): ChatNeedFix {
  if (budget.still_over === "pins" && budget.pinned_by.length > 0) {
    const at = scopeOfPinner(budget.pinned_by[0]);
    return { act: "studio", label: `Open ${skillStudioTitle(at)}`, scope: at, page: "library" };
  }
  if (budget.still_over === "not-hideable" && routing.act === "studio")
    return { act: "studio", label: "Open Harness skills", scope: routing.scope, page: "harness" };
  return routing;
}

/** **What the view's header says of the chat's set** under whose it is: its budget, and how its
 *  router did at its start where it did not fall back (that is a need). */
export function chatSetSaid(said: ChatSkillsSaid): string[] {
  return [
    ...(said.budget === null ? [] : [budgetSaid(said.budget)]),
    ...(said.routing === null || said.routing.fell_back ? [] : [said.routing.said]),
  ];
}

/** The budget in a line: `12 skills of 30 · 3,400 of 6,000 characters`, and the manual-only
 *  skills, free on every chat (SK-8). */
export function budgetSaid(budget: SkillBudget): string {
  const chars = `${budget.chars.toLocaleString("en-US")} of ${budget.most_chars.toLocaleString("en-US")} characters`;
  const skills = `${counted(budget.skills, "skill", "skills")} of ${budget.most_skills}`;
  const manual = counted(budget.manual, "manual-only skill", "manual-only skills");
  const free = budget.manual === 0 ? "" : ` · ${manual} free`;
  const hidden =
    budget.hidden === 0 ? "" : ` · ${counted(budget.hidden, "skill", "skills")} in the index skill`;
  return `${skills} · ${chars}${free}${hidden}`;
}

/**
 * What the chat warns of where its set is at four fifths of the budget or past it, or nothing
 * below: which part, and what can be done.
 */
export function warnsSaid(budget: SkillBudget): string | undefined {
  const what =
    budget.warns === "both"
      ? "skills and their descriptions are"
      : budget.warns === "skills"
        ? "skills are"
        : budget.warns === "chars"
          ? "skills' descriptions are"
          : undefined;
  if (what === undefined) return undefined;
  if (budget.still_over === "not-hideable")
    return `purlis's own skills and the ones this chat's program and repos bring are past purlis's skill budget by themselves, so all of them are listed. Turn some of the program's or repos' skills off in the Skill studio.`;
  if (budget.still_over === "pins")
    return `This chat's pinned skills are past purlis's skill budget, so all of them are listed. Unpin some in ${unpinIn(budget)}, or make some manual-only.`;
  if (budget.hidden > 0)
    return `This chat's ${what} past purlis's skill budget, so ${counted(budget.hidden, "skill is", "skills are")} hidden behind the index skill, which the chat loads them from. Make some skills manual-only, or turn some off for this workspace.`;
  return `This chat's ${what} near or past purlis's skill budget, past which some are hidden behind the index skill. Make some skills manual-only, or turn some off for this workspace.`;
}

/**
 * **A chat whose workspace's and persona's pins pass the most it reads only together** (#3391):
 * each list alone is accepted, so neither studio says anything is wrong, but the chat reads none
 * of either. Names both, how many each pins, and where to unpin; nothing where its pins are read.
 */
export function pinsTogetherSaid(budget: SkillBudget): string | undefined {
  const together = budget.pins_together;
  if (together === null) return undefined;
  const held = together.held
    .map((one) => `${pinnerSaid(one.by)} pins ${counted(one.pins, "skill", "skills")}`)
    .join(" and ");
  const where = studiosOf(together.held.map((one) => one.by));
  return `This chat reads none of its pins: ${held}, more than the ${together.most} a chat reads together. Unpin some in ${where}.`;
}

/** Where a chat's pins are undone (SK-15d): the studio of each of its workspace and persona that
 *  pins a skill, by its tab's title, or the studio itself where the window was not told. */
function unpinIn(budget: SkillBudget): string {
  return budget.pinned_by.length === 0 ? SKILL_STUDIO_TITLE : studiosOf(budget.pinned_by);
}

/** Each pinner's studio, by its tab's title: `Skill studio · ops or Skill studio · reviewer`. */
function studiosOf(pinners: readonly SkillPinner[]): string {
  return pinners.map((by) => skillStudioTitle(scopeOfPinner(by))).join(" or ");
}
