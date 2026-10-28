// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback } from "react";
import { layoutPref } from "./layoutPref";
import { isGroupId, type GroupId } from "./skillsListing";

/**
 * **What the Skills view keeps for the window** (SK-55, #3266; #3308, #3281): whether its list
 * is narrowed to the chat in front, Only this chat's (the decisions amendment's (c)), and which
 * of its groups are folded. One of each for the window, whichever project is in front: a group
 * is a level (Built-in, Workspace) or a program's own, which every project has alike.
 *
 * **Kept in the layout file, under `skills`** (`layout.json`, `regions.ts`), beside the window's
 * other choices on this machine, so it survives a restart as the Chats list's drawing does. Only
 * what is not the default is written, `{ "onlyThisChat": true, "folded": ["built-in"] }`: the
 * whole list, every group open, is what the file leaves out.
 */
export type SkillsViewPrefs = {
  onlyThisChat: boolean;
  /** The groups folded, in the order they were folded; every other group is open. */
  folded: readonly GroupId[];
};

export const DEFAULT_SKILLS_VIEW: SkillsViewPrefs = { onlyThisChat: false, folded: [] };

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
      `"skills" ${JSON.stringify(held)} is not the Skills view's choices, so it lists every skill`,
    ],
  };
  if (held === null || typeof held !== "object" || Array.isArray(held)) return refused;
  const { onlyThisChat: only, folded } = held as { onlyThisChat?: unknown; folded?: unknown };
  if (only !== undefined && typeof only !== "boolean") return refused;
  if (folded !== undefined && !Array.isArray(folded)) return refused;
  // Read as group names and nothing else: a name that is not one is skipped, a second is one.
  const groups = [...new Set((folded ?? []).filter(isGroupId))].slice(-MOST_FOLDED);
  return { value: { onlyThisChat: only === true, folded: groups }, said: [] };
}

/** What the layout document holds of `prefs`: nothing where it is the default. */
function writtenOf(prefs: SkillsViewPrefs): unknown {
  const out: { onlyThisChat?: true; folded?: GroupId[] } = {};
  if (prefs.onlyThisChat) out.onlyThisChat = true;
  if (prefs.folded.length > 0) out.folded = [...prefs.folded];
  return Object.keys(out).length === 0 ? undefined : out;
}

/** The store (`layoutPref.ts`), under `skills`. */
export const SKILLS_VIEW = layoutPref<SkillsViewPrefs, "skills">({
  key: "skills",
  fallback: DEFAULT_SKILLS_VIEW,
  load: loadSkillsView,
  same: (one, other) =>
    one.onlyThisChat === other.onlyThisChat &&
    one.folded.length === other.folded.length &&
    one.folded.every((group, at) => other.folded[at] === group),
  written: writtenOf,
  remedy: (where) =>
    `fix ${where}, or press Only this chat's or fold a group in the Skills view, which rewrites it`,
});

/** Whether the list is narrowed to the chat in front, and the way to change it. */
export function useOnlyThisChat(): [boolean, (only: boolean) => void] {
  return [SKILLS_VIEW.use().onlyThisChat, setOnlyThisChat];
}

function setOnlyThisChat(only: boolean): void {
  SKILLS_VIEW.set({ ...SKILLS_VIEW.value(), onlyThisChat: only });
}

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
    SKILLS_VIEW.set({
      ...SKILLS_VIEW.value(),
      folded: foldedAfter(SKILLS_VIEW.value().folded, shown, open),
    });
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
