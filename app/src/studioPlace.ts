// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { SkillHiddenOn, Studio, StudioClone, StudioSkill } from "./bindings";

/**
 * **Where the previewed chat starts** (#4227; variant A of its prototype): at the workspace's
 * root, or in one of its clones that holds repo skills, which a chat started there counts against
 * its budget too. Every page of the studio follows it: the Library's marks, a skill's page,
 * Harness skills and Routing. The core routes a start in each such clone once per list, from what
 * that list already read (`HarnessInTheStudio::in_clones`), so choosing a place reads nothing:
 * the studio is drawn again from the answer it holds.
 */

/** **The clone `where` names, among those the studio can preview a chat in**, or `undefined`
 *  for a chat at the workspace's root: where nothing is chosen, or the clone chosen holds no repo
 *  skill now, so a chat there lists what one at the root lists. */
export function cloneChosen(
  studio: Pick<Studio, "places"> | undefined,
  where: string | undefined,
): StudioClone | undefined {
  if (where === undefined || studio === undefined) return undefined;
  return studio.places.find((one) => one.clone === where);
}

/** **A skill's hidden marks for a chat started in `clone`**, in the shape the marks at the
 *  root have, so every reader of `hidden` draws them as it is. */
function hiddenIn(skill: StudioSkill, clone: string): SkillHiddenOn[] {
  return skill.hidden_in
    .filter((one) => one.clone === clone)
    .map(({ harness, title, hidden }) => ({ harness, title, hidden }));
}

/**
 * **The studio as a chat started in `where` sees it** (#4227): each row's hidden marks are that
 * chat's start's, and the Routing page's Preview line says that place. At the workspace's root, or
 * for a clone that is no place to preview, the studio as it was read, the same object.
 */
export function studioIn(studio: Studio, where: string | undefined): Studio {
  const clone = cloneChosen(studio, where);
  if (clone === undefined) return studio;
  const mark = (skill: StudioSkill): StudioSkill => ({
    ...skill,
    hidden: hiddenIn(skill, clone.clone),
  });
  return {
    ...studio,
    skills: studio.skills.map(mark),
    harnesses: studio.harnesses.map((harness) => ({
      ...harness,
      skills: harness.skills.map(mark),
    })),
    router: studio.router === null ? null : { ...studio.router, preview: clone.preview },
  };
}

/** **Where a chat starts, as a heading or a list's first word says it**: `At the workspace's
 *  root`, `In purlis`. */
export function placeTitle(clone: string | undefined): string {
  return clone === undefined ? "At the workspace's root" : `In ${clone}`;
}

/** **What a chat in one place does with a skill**, as a skill's page lists it per place. */
export type InAPlace = {
  /** The clone, or `undefined` for the workspace's root. */
  clone: string | undefined;
  said: string;
  /** Whether that chat lists it plainly: the quiet rows. */
  listed: boolean;
};

/** The programs `ones` name, joined: `Claude Code, Codex`. */
function programs(ones: readonly { title: string }[]): string {
  return [...new Set(ones.map((one) => one.title))].join(", ");
}

/**
 * **Whether a skill is listed or hidden in each place** (#4227, the verdict's per-place list,
 * from variant C): at the workspace's root, then in each clone that holds repo skills. `skill` is
 * the row as read, with its marks at the root. A skill a chat does not get is in no place's list,
 * so it has none; nor has a repo skill, which is a chat's only in its own clone.
 */
export function placesOf(skill: StudioSkill, places: readonly StudioClone[]): InAPlace[] {
  if (!skill.on || places.length === 0 || skill.owner?.kind === "repo") return [];
  const hidden = (ones: readonly { title: string }[]) =>
    `Hidden by the router on ${programs(ones)}`;
  const root: InAPlace =
    skill.hidden.length > 0
      ? { clone: undefined, said: hidden(skill.hidden), listed: false }
      : { clone: undefined, said: "Listed", listed: true };
  return [
    root,
    ...places.map(({ clone }): InAPlace => {
      const replaced = skill.replaced_in.filter((one) => one.clone === clone);
      const hid = skill.hidden_in.filter((one) => one.clone === clone);
      const said = [
        ...(replaced.length > 0
          ? [`Replaced by the repo's own ${skill.name} on ${programs(replaced)}`]
          : []),
        ...(hid.length > 0 ? [hidden(hid)] : []),
      ];
      return said.length === 0
        ? { clone, said: "Listed", listed: true }
        : { clone, said: said.join("; "), listed: false };
    }),
  ];
}
