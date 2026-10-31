// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback } from "react";
import { layoutPref } from "./layoutPref";
import { isGroupId, type GroupId } from "./skillsListing";

/**
 * **What the Skills view keeps for the window** (SK-55, #3266; #3308, #3281; spec #3679 S7):
 * which of its groups are folded. One for the window, whichever project is in front: a group is
 * Needs you, In this chat, Left out or the built-in skills' quiet line, which every project has
 * alike. The view follows the chat in front, so it keeps no choice of whose skills it lists: the
 * Only this chat's an older purlis kept (`onlyThisChat`) is read past and never written.
 *
 * **Kept in the layout file, under `skills`** (`layout.json`, `regions.ts`), beside the window's
 * other choices on this machine, so it survives a restart as the Chats list's drawing does. Only
 * what is not the default is written, `{ "folded": ["built-in"] }`: Left out and the built-in
 * skills folded is what the file leaves out.
 */
export type SkillsViewPrefs = {
  /** The groups folded, in the order they were folded; every other group is open. */
  folded: readonly GroupId[];
};

/** Left out and the built-in skills folded, the rest open (S7, S12). */
export const DEFAULT_SKILLS_VIEW: SkillsViewPrefs = { folded: ["built-in", "left-out"] };

/** The most folds kept: more groups than a list draws, so a hand-edited file cannot grow it. */
export const MOST_FOLDED = 16;

/** The preferences as a layout document holds them, and what had to be put right. */
export function loadSkillsView(raw: unknown): { value: SkillsViewPrefs; said: string[] } {
  const held =
    raw !== null && typeof raw === "object" && !Array.isArray(raw)
      ? (raw as { skills?: unknown }).skills
      : undefined;
  if (held === undefined) return { value: DEFAULT_SKILLS_VIEW, said: [] };
  const refused = {
    value: DEFAULT_SKILLS_VIEW,
    said: [
      `"skills" ${JSON.stringify(held)} is not the Skills view's choices, so its groups fold as they start`,
    ],
  };
  if (held === null || typeof held !== "object" || Array.isArray(held)) return refused;
  const { folded } = held as { folded?: unknown };
  if (folded === undefined) return { value: DEFAULT_SKILLS_VIEW, said: [] };
  if (!Array.isArray(folded)) return refused;
  // Read as group names and nothing else: a name that is not one is skipped, a second is one.
  const groups = [...new Set(folded.filter(isGroupId))].slice(-MOST_FOLDED);
  // Folds an older purlis kept of its levels are no group now: the groups fold as they start,
  // never all open because nothing kept was read.
  if (groups.length === 0 && folded.length > 0) return { value: DEFAULT_SKILLS_VIEW, said: [] };
  return { value: { folded: groups }, said: [] };
}

/** Whether two lists of folds are one. */
function sameFolds(one: readonly GroupId[], other: readonly GroupId[]): boolean {
  return one.length === other.length && one.every((group, at) => other[at] === group);
}

/** What the layout document holds of `prefs`: nothing where it is the default. */
function writtenOf(prefs: SkillsViewPrefs): unknown {
  return sameFolds(prefs.folded, DEFAULT_SKILLS_VIEW.folded)
    ? undefined
    : { folded: [...prefs.folded] };
}

/** The store (`layoutPref.ts`), under `skills`. */
export const SKILLS_VIEW = layoutPref<SkillsViewPrefs, "skills">({
  key: "skills",
  fallback: DEFAULT_SKILLS_VIEW,
  load: loadSkillsView,
  same: (one, other) => sameFolds(one.folded, other.folded),
  written: writtenOf,
  remedy: (where) => `fix ${where}, or fold a group in the Skills view, which rewrites it`,
});

/**
 * **The groups folded, and the way to fold or open some**: `shown` are the groups drawn now and
 * `open` the ones of them left open. A group not drawn now keeps its fold, so a Needs you that
 * empties and fills again comes back as it was left.
 */
export function useFoldedGroups(): [
  readonly GroupId[],
  (shown: readonly GroupId[], open: ReadonlySet<GroupId>) => void,
] {
  const { folded } = SKILLS_VIEW.use();
  const fold = useCallback((shown: readonly GroupId[], open: ReadonlySet<GroupId>) => {
    SKILLS_VIEW.set({ folded: foldedAfter(SKILLS_VIEW.value().folded, shown, open) });
  }, []);
  return [folded, fold];
}

/** The folds after the groups `shown` were left with `open` open: the newest fold last, and the
 *  oldest let go past {@link MOST_FOLDED}. */
export function foldedAfter(
  was: readonly GroupId[],
  shown: readonly GroupId[],
  open: ReadonlySet<GroupId>,
): GroupId[] {
  const kept = was.filter((group) => !shown.includes(group) || !open.has(group));
  const added = shown.filter((group) => !open.has(group) && !kept.includes(group));
  return [...kept, ...added].slice(-MOST_FOLDED);
}
