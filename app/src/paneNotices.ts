// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A pane's notices, minimised or expanded** (#4754, spec #4750 stories 33 to 39, the accepted
 * prototype `prototype/pane-notices`, variant A).
 *
 * - **One choice for every pane**, minimised by default, kept across restarts in the layout file
 *   under `notices` (`layout.json`, `regions.ts`), beside the window's other choices on this
 *   machine. Only `expanded` is written: minimised is what the file leaves out.
 * - **Minimised, a pane's notes fold into its bell's count**, and its asks (a consent, a blocking
 *   question) stay as full cards in the band, since an ask must keep its words.
 * - **Expanded, the band shows every notice**, with Dismiss all on its head row, which skips the
 *   asks and counts what it clears.
 * - **The bell's count is the pane's notes**, never its asks, the same number in both states.
 *
 * What is said, here, with no import: the notice family's band (`Notice.tsx`) draws it, and the
 * theme specimen draws that without the window's state. The choice itself is kept in
 * `paneNoticesPref.ts`.
 */
export type PaneNoticesShown = "minimised" | "expanded";

/** **What a pane's band holds**, as its bell says it: its notes, how many of them are trouble,
 *  and its asks, which are never counted. */
export type BandHolds = { notes: number; trouble: number; asks: number };

export const HOLDS_NOTHING: BandHolds = { notes: 0, trouble: 0, asks: 0 };

/** Whether two holds are one, so the band tells its bell only of a change. */
export const sameHolds = (one: BandHolds, other: BandHolds): boolean =>
  one.notes === other.notes && one.trouble === other.trouble && one.asks === other.asks;

/** **The bell's count**: the pane's notes, and nothing where it has none. */
export const bellCount = (holds: BandHolds): string => (holds.notes === 0 ? "" : `${holds.notes}`);

const plural = (count: number, one: string, many: string) => (count === 1 ? one : many);

/**
 * **What the bell says to a screen reader**, and what is announced when a notice arrives:
 * "5 notices, 1 in trouble, 2 asking". The trouble is said only where there is some, and the
 * asks only where there are some.
 */
export function bellSays(holds: BandHolds): string {
  const notes =
    holds.notes === 0 ? "No notices" : `${holds.notes} ${plural(holds.notes, "notice", "notices")}`;
  const trouble = holds.trouble > 0 ? `, ${holds.trouble} in trouble` : "";
  const asks = holds.asks > 0 ? `, ${holds.asks} asking` : "";
  return `${notes}${trouble}${asks}`;
}

/** **What the band's head row says, expanded**: how many it holds, and how many wait on you. */
export function bandHeadSays(holds: BandHolds): string {
  const all = holds.notes + holds.asks;
  const waiting = holds.asks > 0 ? `, ${holds.asks} waiting on you` : "";
  return `${all} ${plural(all, "notice", "notices")}${waiting}`;
}

/** **Dismiss all's words**: it counts what it clears, the notes with a Dismiss of their own. */
export const dismissAllSays = (clears: number): string => `Dismiss all ${clears}`;

/** Whether a notice arrived: the band holds more than it did. Only then is the count said. */
export const arrived = (was: BandHolds, now: BandHolds): boolean =>
  now.notes + now.asks > was.notes + was.asks;
