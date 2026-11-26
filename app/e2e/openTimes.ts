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
 *
 * **A swapping runner is not judged on speed.** Each open records the runner (swap, free memory,
 * load; `judgeOpens`). When swap in use grew by more than `SWAP_GROWTH_MB` across the run, a
 * median or p90 over its limit is *inconclusive*, not red: the slowdown tracks the machine's
 * memory. A hang and an empty run fail regardless. The opens are timed with the cheap
 * `harnessesRunningCheaply` probe (250 ms), so the limits' basis (older runs, 100 ms probe with
 * two full `ps` listings) was, if anything, measured slower than what is read now.
 */

/** A start that is slower than this on its own is a hang, not load: it fails by its number. */
export const OPEN_HANG_MS = 20_000;
/**
 * What the median open stays under: the CI median plus 20% headroom, from measured runs, not a
 * guess (#7404's 5,000 was one, and failed twice). Basis: `main`, run 45144506452 (macos-latest,
 * part 2, job 137590652671), both attempts failed on the median alone, each open steady between
 * 4.6 and 7.4 s: attempt 1 median 5,586 ms (p90 6,797, slowest 7,387); attempt 2 median 5,545 ms
 * (p90 6,191, slowest 7,131). 5,586 x 1.2 = 6,703, so 6,700. The p90 (8,000) and the hang (20,000)
 * limits hold: the worst measured p90 is 6,797 and the worst open 7,387.
 */
export const OPEN_MEDIAN_MS = 6_700;
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

/** What the runner looked like at one open: what a swapping machine shows and a slow product does not. */
export interface RunnerSample {
  /** Swap in use, in MB; `null` when the platform would not say. */
  swapMb: number | null;
  /** Free memory, in MB. */
  freeMb: number;
  /** The one-minute load average. */
  load: number;
}

/** One open: how long it took and the runner at that moment. */
export interface Open {
  ms: number;
  runner: RunnerSample | null;
}

/** Swap that grew by more than this across the run (MB) says the runner was swapping, not the product. */
export const SWAP_GROWTH_MB = 256;

/** `sysctl vm.swapusage` on macOS: "vm.swapusage: total = 2048.00M  used = 1180.25M  free = 867.75M ...". */
export function parseSwapUsedMb(text: string): number | null {
  const found = /used\s*=\s*([\d.]+)\s*([KMG])/i.exec(text);
  if (found === null) return null;
  const unit = { K: 1 / 1024, M: 1, G: 1024 }[found[2].toUpperCase() as "K" | "M" | "G"];
  return Math.round(Number.parseFloat(found[1]) * unit * 100) / 100;
}

/** Whether swap use grew across the opens by more than {@link SWAP_GROWTH_MB}. */
export function runnerWasSwapping(opens: readonly Open[], before: number | null = null): boolean {
  const swaps = (before === null ? [] : [before]).concat(
    opens.flatMap((open) => (open.runner?.swapMb == null ? [] : [open.runner.swapMb])),
  );
  return swaps.length >= 2 && Math.max(...swaps) - swaps[0] > SWAP_GROWTH_MB;
}

/** The one line a failing or inconclusive run prints about the runner. */
export function runnerSummary(opens: readonly Open[]): string {
  const seen = opens.flatMap((open) => (open.runner === null ? [] : [open.runner]));
  if (seen.length === 0) return "runner: not sampled";
  const first = seen[0];
  const last = seen[seen.length - 1];
  const swap = (r: RunnerSample) => (r.swapMb === null ? "n/a" : `${r.swapMb} MB`);
  return (
    `runner: swap ${swap(first)} -> ${swap(last)}, free memory ${first.freeMb} -> ${last.freeMb} MB, ` +
    `load ${first.load} -> ${last.load}`
  );
}

/** How the opens fared: held, failed, or too disturbed by the runner to say about speed. */
export interface OpensJudgment {
  state: "held" | "failed" | "inconclusive";
  said: string | null;
}

/**
 * Judges the opens with the runner beside them. A hang fails always. A median or p90 over its
 * limit fails on a runner that was not swapping, and is **inconclusive** on one that was: the
 * slowdown then tracks the machine's memory, and a red `main` would blame the product for it.
 */
export function judgeOpens(
  opens: readonly Open[],
  swapBefore: number | null = null,
): OpensJudgment {
  const samples = opens.map((open) => open.ms);
  const trouble = opensVerdict(samples);
  if (trouble === null) return { state: "held", said: null };
  const said = `${trouble}; ${runnerSummary(opens)}`;
  // By the numbers, not the message's words: a hang, or nothing timed, always fails.
  if (samples.length === 0 || Math.max(...samples) > OPEN_HANG_MS) {
    return { state: "failed", said };
  }
  if (runnerWasSwapping(opens, swapBefore)) {
    return {
      state: "inconclusive",
      said: `the runner was swapping, so the open times say nothing about the app: ${said}`,
    };
  }
  return { state: "failed", said };
}
