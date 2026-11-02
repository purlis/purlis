// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The five rows for what the operator feels** (#4744, rows L11 to L15 of the spec's budgets of
 * record), and how a run is held to each.
 *
 * `bench/felt.bench.ts` drives the window; this file only reads the numbers it took, so the
 * reading can be tested without an app (`switchTimes.ts` is split from `switching.ts` the same
 * way). Each limit is the one an existing row already holds the same thing to, named beside it.
 */

/** A row held to a time at the p95 of its samples. */
export type Row = { id: string; budgetMs: number };

export const ROWS = {
  /** L12: a chat switch with fifty chats busy, held to L2's tab switch. */
  switchAtFiftyBusy: { id: "L12", budgetMs: 100 },
  /** L13: a keystroke in the first second after a switch, held to L1's keystroke to screen. */
  echoAfterASwitch: { id: "L13", budgetMs: 50 },
  /** L14: the new-chat dialog's open, held to L2's: it opens at once (#4724, item 6). */
  dialogOpen: { id: "L14", budgetMs: 100 },
} as const satisfies Record<string, Row>;

/** L11: a frame this long is two at a 60 fps display, L4's 30 fps floor. */
export const FRAME_P95_MS = 33.4;

/** L11: a gap this long is a stall, as the slow log counts one (`src/slowLog.ts`, #4725). */
export const STALL_MS = 200;

function percentile(sorted: readonly number[], at: number): number {
  return sorted[Math.min(sorted.length - 1, Math.floor((at / 100) * sorted.length))];
}

/** A row's numbers and its verdict, as `tools/window-verdicts.mjs` prints them. */
export type Held = {
  budgetMs: number;
  samples: number;
  p50: number;
  p95: number;
  worst: number;
  met: boolean;
  samples_ms: number[];
};

/** Holds `samples` to `row`: met when there are some and their p95 is within its budget. */
export function heldTo(row: Row, samples: readonly number[]): Held {
  const sorted = [...samples].sort((a, b) => a - b);
  const p95 = percentile(sorted, 95);
  return {
    budgetMs: row.budgetMs,
    samples: sorted.length,
    p50: percentile(sorted, 50),
    p95,
    worst: sorted[sorted.length - 1],
    met: sorted.length > 0 && p95 <= row.budgetMs,
    samples_ms: [...samples],
  };
}

/**
 * L11, frame time while twenty agents stream in the background: met when the p95 gap between
 * frames is within {@link FRAME_P95_MS} and no gap was a stall.
 */
export function frameTime(gapsMs: readonly number[]): {
  frames: number;
  p50Ms: number;
  p95Ms: number;
  longestMs: number;
  stalls: number;
  met: boolean;
} {
  const sorted = [...gapsMs].sort((a, b) => a - b);
  const p95Ms = percentile(sorted, 95);
  const stalls = sorted.filter((gap) => gap >= STALL_MS).length;
  return {
    frames: sorted.length,
    p50Ms: percentile(sorted, 50),
    p95Ms,
    longestMs: sorted[sorted.length - 1],
    stalls,
    met: sorted.length > 0 && p95Ms <= FRAME_P95_MS && stalls === 0,
  };
}

/** An answer of 1 s or longer, as the slow log wrote it down. */
export type Late = { command: string; ms: number; hot: number };

/**
 * L15, no late answer during a run: read off the window's slow log (`slow.jsonl`, #4725), which
 * writes down every command answer of 1 s or longer and every stall of 200 ms or longer. Only
 * what was written at or after `since` (ms since the epoch) is the run's. A stall is evidence
 * beside the row, counted and not judged: L11 holds the frames.
 *
 * @param log the file's text, or `undefined` where the window wrote none, which is no late answer.
 */
export function lateAnswers(
  log: string | undefined,
  since: number,
): { met: boolean; answers: Late[]; stalls: number } {
  const answers: Late[] = [];
  let stalls = 0;
  for (const line of (log ?? "").split("\n")) {
    let moment: { kind?: unknown; command?: unknown; ms?: unknown; at?: unknown; hot?: unknown };
    try {
      moment = JSON.parse(line) as typeof moment;
    } catch {
      // A line cut short by a write in progress, or the empty one after the last newline.
      continue;
    }
    if (typeof moment.at !== "number" || moment.at < since) continue;
    if (moment.kind === "stall") stalls += 1;
    if (moment.kind === "answer")
      answers.push({
        command: String(moment.command),
        ms: Number(moment.ms),
        hot: Number(moment.hot),
      });
  }
  return { met: answers.length === 0, answers, stalls };
}
