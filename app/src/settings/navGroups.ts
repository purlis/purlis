// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { layoutPref } from "../layoutPref";

/**
 * **Which of Settings' page groups are open** (#2919, prototype A, R47): a page with pages of its
 * own (Sandbox, Harness & profiles) is a group in the nav, open or folded. One choice for the
 * window, by each group's address, whichever level or project is shown.
 *
 * **Every group starts open**, Sandbox among them: the prototype's one group, which it opened
 * (R47). What the person folds is kept, in the layout file, under `settings` (`layout.json`,
 * `regions.ts`), beside the window's other choices on this machine, so it survives a restart.
 * Only what is not the default is written, `{ "folded": ["project.harness"] }`: nothing folded is
 * what the file leaves out.
 */
export type SettingsGroupsPrefs = {
  /** The groups folded, in the order they were folded; every other group is open. */
  folded: readonly string[];
};

/** Every group open (R47). */
export const DEFAULT_GROUPS: SettingsGroupsPrefs = { folded: [] };

/** The most kept folded: more groups than Settings has, so a hand-edited file cannot grow it. */
export const MOST_FOLDED = 32;

/** A group's address as Settings mints one: `<level>.<group>`, its parts lowercase words. */
const ADDRESS = /^[a-z][a-z0-9-]*(\.[a-z0-9][a-z0-9_-]*)+$/;

/** The preferences as a layout document holds them, and what had to be put right. */
export function loadSettingsGroups(raw: unknown): { value: SettingsGroupsPrefs; said: string[] } {
  const held =
    raw !== null && typeof raw === "object" && !Array.isArray(raw)
      ? (raw as { settings?: unknown }).settings
      : undefined;
  if (held === undefined) return { value: DEFAULT_GROUPS, said: [] };
  const refused = {
    value: DEFAULT_GROUPS,
    said: [
      `"settings" ${JSON.stringify(held)} is not Settings' folded groups, so every group is open`,
    ],
  };
  if (held === null || typeof held !== "object" || Array.isArray(held)) return refused;
  const { folded } = held as { folded?: unknown };
  if (folded === undefined) return { value: DEFAULT_GROUPS, said: [] };
  if (!Array.isArray(folded)) return refused;
  // Read as addresses and nothing else: one that is not is skipped, a second is one.
  const groups = [
    ...new Set(folded.filter((one): one is string => typeof one === "string" && ADDRESS.test(one))),
  ].slice(-MOST_FOLDED);
  return { value: { folded: groups }, said: [] };
}

function sameFolds(one: readonly string[], other: readonly string[]): boolean {
  return one.length === other.length && one.every((group, at) => other[at] === group);
}

/** The store (`layoutPref.ts`), under `settings`. */
export const SETTINGS_GROUPS = layoutPref<SettingsGroupsPrefs, "settings">({
  key: "settings",
  fallback: DEFAULT_GROUPS,
  load: loadSettingsGroups,
  same: (one, other) => sameFolds(one.folded, other.folded),
  written: (prefs) => (prefs.folded.length === 0 ? undefined : { folded: [...prefs.folded] }),
  remedy: (where) => `fix ${where}, or open or fold a group in Settings' pages, which rewrites it`,
});

/** Whether `group` is open now, by what is kept. */
export function isOpen(prefs: SettingsGroupsPrefs, group: string): boolean {
  return !prefs.folded.includes(group);
}

/** Opens or folds `group`, and writes the choice to the layout file. */
export function setGroupOpen(group: string, open: boolean): void {
  const now = SETTINGS_GROUPS.value();
  if (open === isOpen(now, group)) return;
  SETTINGS_GROUPS.set({
    folded: open ? now.folded.filter((one) => one !== group) : [...now.folded, group],
  });
}

/** {@link SETTINGS_GROUPS} as `SettingsLayout`'s `folds`, for a component that redraws when it
 *  changes. */
export function useFolds(): { isOpen: (group: string) => boolean; setOpen: typeof setGroupOpen } {
  const kept = SETTINGS_GROUPS.use();
  return { isOpen: (group) => isOpen(kept, group), setOpen: setGroupOpen };
}
