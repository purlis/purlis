// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { PlaneUpdated, SkillBudget, SkillRouting, SkillsMissing } from "./bindings";
import { counted } from "./dispatches";
import { missingSays } from "./SkillsMissingNotice";
import {
  AT_THE_ROOT,
  asPersona,
  scopeOfPinner,
  skillStudioTitle,
  type StudioScope,
} from "./skillStudio";
import { budgetSaid, pinsTogetherSaid, warnsSaid } from "./UseSkill";

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
  | { act: "studio"; label: string; scope: StudioScope; page: "library" | "routing" }
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
    label: "Open Routing",
    scope,
    page: "routing" as const,
  };
  const budget = said.budget;
  const warns = budget === null ? undefined : warnsSaid(budget);
  if (budget !== null && warns !== undefined)
    needs.push({ id: "budget", title: "Near the skill budget", says: warns, fix: routing });
  const together = budget === null ? undefined : pinsTogetherSaid(budget);
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
    const count =
      missing.missing.length +
      missing.no_built_in.length +
      missing.broken.length +
      missing.by_a_repo.length +
      missing.beside_a_built_in.length;
    needs.push({
      id: "missing",
      title: `Started without ${counted(count, "skill", "skills")}`,
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

/** **What the view's header says of the chat's set** under whose it is: its budget, and how its
 *  router did at its start where it did not fall back (that is a need). */
export function chatSetSaid(said: ChatSkillsSaid): string[] {
  return [
    ...(said.budget === null ? [] : [budgetSaid(said.budget)]),
    ...(said.routing === null || said.routing.fell_back ? [] : [said.routing.said]),
  ];
}
