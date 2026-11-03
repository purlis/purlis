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
  /**
   * L12: a switch to a chat that is not among the last {@link KEPT} shown and has just taken a
   * burst, with fifty chats busy, held to L2's tab switch.
   */
  switchAtFiftyBusy: { id: "L12", budgetMs: 100 },
  /** L13: a keystroke in the first second after a switch, held to L1's keystroke to screen. */
  echoAfterASwitch: { id: "L13", budgetMs: 50 },
  /** L14: the new-chat dialog's open, held to L2's: it opens at once (#4724, item 6). */
  dialogOpen: { id: "L14", budgetMs: 100 },
} as const satisfies Record<string, Row>;

/** L11: a frame this long is two at a 60 fps display, L4's 30 fps floor (`docs/spec.md`). */
export const FRAME_P95_MS = 33;

/**
 * The fewest samples a p95 is read from. Of fewer than forty the p95 is the worst sample, and a
 * row that says p95 would be holding the worst.
 */
export const P95_SAMPLES = 40;

/** How many chats the window keeps the terminal of once shown (#4724, item 5). */
export const KEPT = 3;

/** A target's burst, as `--loops` of the recorded corpus: the spec's 2 MB and 13 MB. */
export const BURSTS = { "2 MB": 16, "13 MB": 99 } as const;

/** One of L12's switches: what `target` is sent first, and the chats shown before going back. */
export type Switch = { target: number; loops: number; away: number[] };

/** The burst `target` is sent each time, as its harness is started with: 2 MB and 13 MB in turn. */
export function burstOf(target: number): number {
  return target % 2 === 0 ? BURSTS["2 MB"] : BURSTS["13 MB"];
}

/**
 * **L12's switches** (#4744): over `targets` chats in turn, each sent its burst ({@link burstOf})
 * and then left for {@link KEPT} of the `streaming` chats, so the switch back is to a chat whose
 * terminal is no longer kept and whose output arrived while it was hidden: the operator's switch
 * to a chat that was working, not back to the one just left. An even number of targets, so the
 * switches alternate 2 MB and 13 MB.
 */
export function switchPlan(samples: number, targets: number, streaming: number): Switch[] {
  if (targets <= KEPT) throw new Error(`${targets} targets: one would always be kept`);
  if (targets % 2 !== 0) throw new Error(`${targets} targets: the bursts would not alternate`);
  if (streaming < KEPT) throw new Error(`${streaming} streaming: too few to leave a target for`);
  return Array.from({ length: samples }, (_, n) => ({
    target: n % targets,
    loops: burstOf(n % targets),
    away: Array.from({ length: KEPT }, (_, i) => (n * KEPT + i) % streaming),
  }));
}

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

/**
 * Holds `samples` to `row`: met when there are at least {@link P95_SAMPLES} and their p95 is
 * within its budget.
 */
export function heldTo(row: Row, samples: readonly number[]): Held {
  const sorted = [...samples].sort((a, b) => a - b);
  const p95 = percentile(sorted, 95);
  return {
    budgetMs: row.budgetMs,
    samples: sorted.length,
    p50: percentile(sorted, 50),
    p95,
    worst: sorted[sorted.length - 1],
    met: sorted.length >= P95_SAMPLES && p95 <= row.budgetMs,
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

/** An answer of 1 s or longer, as the slow log wrote it down, and the phase it fell in. */
export type Late = { command: string; ms: number; hot: number; phase: string };

/** A stretch of the run that measured a row, in ms since the epoch: the bench's setup is not. */
export type Phase = { name: string; from: number; to: number };

/**
 * L15, no late answer while the rows were measured: read off the window's slow log
 * (`slow.jsonl`, #4725), which writes down every command answer of 1 s or longer and every stall
 * of 200 ms or longer. Only what was written inside one of `phases` counts: what the bench asked
 * to set a phase up (fifty chats started, a project opened) is not what the operator waits on. A
 * stall is evidence beside the row, counted and not judged: L11 holds the frames.
 *
 * @param log the file's text, or `undefined` where the window wrote none: not met, since a log
 *   that was never written says nothing about the answers (the bench makes a stall first, so a
 *   window that writes its log has one).
 */
export function lateAnswers(
  log: string | undefined,
  phases: readonly Phase[],
): { met: boolean; logWritten: boolean; answers: Late[]; stalls: number } {
  if (log === undefined) return { met: false, logWritten: false, answers: [], stalls: 0 };
  const answers: Late[] = [];
  let stalls = 0;
  for (const line of log.split("\n")) {
    let moment: { kind?: unknown; command?: unknown; ms?: unknown; at?: unknown; hot?: unknown };
    try {
      moment = JSON.parse(line) as typeof moment;
    } catch {
      // A line cut short by a write in progress, or the empty one after the last newline.
      continue;
    }
    const at = moment.at;
    if (typeof at !== "number") continue;
    const phase = phases.find(({ from, to }) => from <= at && at <= to);
    if (phase === undefined) continue;
    if (moment.kind === "stall") stalls += 1;
    if (moment.kind === "answer")
      answers.push({
        command: String(moment.command),
        ms: Number(moment.ms),
        hot: Number(moment.hot),
        phase: phase.name,
      });
  }
  return { met: answers.length === 0, logWritten: true, answers, stalls };
}
