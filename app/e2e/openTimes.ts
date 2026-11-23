// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **How the fifty-sessions opening's times are judged** (#2157; `specs/panes.e2e.ts`).
 *
 * One New tab's time on a 3-core macOS runner is a noisy number: it is about 3.4 s there, and a
 * runner that swaps now and then takes 8 to 9 s for one open and not the next (2026-11-23: opens
 * 4, 6, 8 and 45 of four runs took 8,052 to 8,822 ms, on a `main` and a queue run that passed on
 * re-run). A line on every single open turned that noise into a red `main`, and the one failure
 * hid the rest of the file's results behind it. So the opens are judged as a set:
 *
 * - **the median** is what a start usually costs, and a start that got slower for every open
 *   moves it: this is the guard that catches a regression;
 * - **the p90** is what a bad open costs: a few slow opens pass, a run that is slow often does not;
 * - **a hang** is one open that took longer than any loaded runner explains, and fails alone.
 */

/** A start that is slower than this on its own is a hang, not load: it fails by its number. */
export const OPEN_HANG_MS = 20_000;
/** What the median open stays under: about 3.4 s measured, so this is half again above it. */
export const OPEN_MEDIAN_MS = 5_000;
/**
 * What the p90 of the opens stays under: the old per-open ceiling. Of fifty opens, the four
 * slowest may pass it (nearest rank, so the fifth slowest is the p90).
 */
export const OPEN_P90_MS = 8_000;

/** The `fraction` of `samples` by nearest rank: the smallest sample that `fraction` are at or below. */
function rank(samples: readonly number[], fraction: number): number {
  const ordered = [...samples].sort((a, b) => a - b);
  return ordered[Math.max(0, Math.ceil(ordered.length * fraction) - 1)];
}

/** What the opens showed, and the sentence a failure carries; `null` when they hold. */
export function opensVerdict(samples: readonly number[]): string | null {
  if (samples.length === 0) return "no open was timed, so none can be judged";
  const slowest = Math.max(...samples);
  const median = rank(samples, 0.5);
  const p90 = rank(samples, 0.9);
  const said =
    `over ${samples.length} opens: median ${median} ms (limit ${OPEN_MEDIAN_MS}), ` +
    `p90 ${p90} ms (limit ${OPEN_P90_MS}), slowest ${slowest} ms (hang at ${OPEN_HANG_MS}); ` +
    `each open: ${samples.join(", ")}`;
  if (slowest > OPEN_HANG_MS) return `an open hung, ${said}`;
  if (median > OPEN_MEDIAN_MS) return `a start is slower than it was, ${said}`;
  if (p90 > OPEN_P90_MS) return `too many opens were slow, ${said}`;
  return null;
}
