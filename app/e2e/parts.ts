// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Which scenario specs a part of a split run runs** (#3615), for `wdio.conf.ts`.
 *
 * WebdriverIO's own `--shard` cuts the name-ordered list into parts of equal COUNT, and the specs
 * are far from equal in time: `panes`, `pane-warnings` and `project-switch` alone take 5.8 of the
 * run's 30 minutes on macOS, and they sit side by side. Four equal-count parts measured 4.6, 10.8,
 * 6.3 and 8.2 minutes (five runs of 2026-10-28), so the longest would barely have moved.
 *
 * So a part is the specs whose names fall between two CUTS, chosen by measured time: still a run
 * of neighbours in name order, as WebdriverIO's parts were, so no spec meets predecessors it has
 * never run after. A cut is a name, not a file: a spec added, renamed or removed lands in exactly
 * one part, and a cut never goes stale into a missing file. When a part grows past the others,
 * the `Annotate the slowest scenario specs` notices say which, and a cut moves.
 *
 * `projects` is a cut in every split on purpose: the Skill studio's draft specs pass only after
 * the specs from `projects` on have run in the same app (#3562), and every part that holds them
 * starts there, as WebdriverIO's second half did.
 */
const CUTS: Record<number, readonly string[]> = {
  // WebdriverIO's halves, as they cut 100 specs (Linux's two parts).
  2: ["projects"],
  // 7.7, 7.7, 6.9 and 7.6 minutes of specs on macOS, the mean of five runs on 2026-10-28.
  4: ["pane-notices", "projects", "tab-chip"],
};

/**
 * The spec files part `part` ("k/n") runs, in the order the app runs them; every file when
 * `part` is not given.
 */
export function specsOfPart(files: readonly string[], part: string | undefined): string[] {
  // By UTF-16 code unit, as WebdriverIO's list came: `skill-studio-drafts` before `skill-studio`.
  const ordered = [...files].sort();
  if (part === undefined) return ordered;
  const asked = /^(\d+)\/(\d+)$/.exec(part);
  if (!asked) throw new Error(`a part is k/n, not "${part}"`);
  const k = Number(asked[1]);
  const n = Number(asked[2]);
  const cuts = CUTS[n];
  if (!cuts) throw new Error(`no cuts for ${n} parts: add them to e2e/parts.ts`);
  if (k < 1 || k > n) throw new Error(`there is no part ${k} of ${n}`);
  const from = k === 1 ? undefined : cuts[k - 2];
  const until = k === n ? undefined : cuts[k - 1];
  return ordered.filter(
    (name) => (from === undefined || name >= from) && (until === undefined || name < until),
  );
}
