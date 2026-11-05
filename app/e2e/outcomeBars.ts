// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What the Skill studio's outcome bars ask, and how a measurement is judged** (SK-23, #2786;
 * #4343; stories 10 and 43 of #2762). The window's side is
 * `specs/skill-outcome-bars.outcome.e2e.ts`.
 *
 * A bar here is the spec's number. A run that misses one fails, naming the bar and what it
 * measured, and the miss gets a ticket: the number does not move to fit (the operator's ruling of
 * 2026-10-31).
 * The 300 ms bar is judged on a real Mac, and CI guards its median instead (the person's ruling of
 * 2026-11-05, ADR 0086's amendment of that date).
 */

/** Story 10: the studio opens in under 300 ms with 200 skills. */
export const OPEN_BUDGET_MS = 300;
/**
 * **CI's guard on the same openings** (#4351, ADR 0086's amendment of 2026-11-05): the median a
 * harness's openings stay under on macOS CI. Coarse on purpose: the same code's medians ran from
 * 214 to 649 ms between runs, so this sits about a quarter above that worst, and it catches a
 * return to the 880 to 2,300 ms of most runs before #4351's fixes, not a small slip.
 */
export const OPEN_GUARD_MEDIAN_MS = 800;
/** Story 10's load. */
export const SKILLS_AT_THE_BAR = 200;
/** How many openings the p95 is of. */
export const OPENINGS = 20;
/** Story 43: under 2 minutes from wanting a skill to using it in a running chat. */
export const USE_BUDGET_MS = 120_000;

/**
 * **What a person spends on one press**, in ms, by the keystroke-level model (Card, Moran and
 * Newell, 1980): deciding (1,350), pointing (1,100) and the press itself (200).
 */
const A_PRESS_MS = 2650;
/** **What a person spends on one typed character**: the model's average typist. */
const A_KEY_MS = 280;

/** What the way to a skill asks of a person's hands. */
export type Effort = { presses: number; characters: number };

/**
 * **What a person spends on `effort`, on top of the window's waits.** A driver presses as fast as
 * the window answers, so its clock alone would pass any number of steps: this is what makes the
 * bar about a person. The real run before the flip times a real one (the pull request of #4343).
 */
export function aPersonsTime({ presses, characters }: Effort): number {
  return presses * A_PRESS_MS + characters * A_KEY_MS;
}

/** A bar's verdict, and the sentence a run prints and a failure carries. */
export type Verdict = { met: boolean; said: string };

/** **The 2 minute bar**: the driver's clock, which holds the window's waits, plus a person's time. */
export function useVerdict(windowMs: number, effort: Effort): Verdict & { totalMs: number } {
  const totalMs = windowMs + aPersonsTime(effort);
  const seconds = (ms: number) => `${(ms / 1000).toFixed(1)} s`;
  return {
    met: totalMs < USE_BUDGET_MS,
    totalMs,
    said:
      `wanting a skill to using it: ${seconds(totalMs)} against ${seconds(USE_BUDGET_MS)} ` +
      `(${seconds(windowMs)} on the driver's clock, ${effort.presses} presses and ` +
      `${effort.characters} characters for a person)`,
  };
}

/** The openings, quickest first; there have to be some to judge. */
function inOrder(samples: readonly number[]): number[] {
  if (samples.length === 0) throw new Error("the studio's opening has no samples to judge");
  return [...samples].sort((a, b) => a - b);
}

/** **The 300 ms bar**: the p95 of the openings, by nearest rank, as row L10 states it. */
export function openVerdict(samples: readonly number[]): Verdict & { p95: number } {
  const ordered = inOrder(samples);
  const p95 = ordered[Math.ceil(ordered.length * 0.95) - 1];
  return {
    met: p95 < OPEN_BUDGET_MS,
    p95,
    said:
      `the studio opening with ${SKILLS_AT_THE_BAR} skills: p95 ${Math.round(p95)} ms against ` +
      `${OPEN_BUDGET_MS} ms, over ${ordered.length} openings ` +
      `(${ordered.map((one) => Math.round(one)).join(", ")})`,
  };
}

/** The middle of `samples`, or the mean of the two in the middle of an even count. */
function medianOf(samples: readonly number[]): number {
  const ordered = inOrder(samples);
  const above = Math.floor(ordered.length / 2);
  return ordered.length % 2 === 1 ? ordered[above] : (ordered[above - 1] + ordered[above]) / 2;
}

/**
 * **What the openings are judged against where they were measured** (#4351): a real machine against
 * the 300 ms bar, macOS CI against the median guard, and Linux CI against nothing yet, since no
 * run there has measured the current code to set a limit from.
 */
export function openJudged(
  samples: readonly number[],
  { ci, platform }: { ci: boolean; platform: string },
): Verdict {
  if (!ci) return openVerdict(samples);
  const median = medianOf(samples);
  const what = `the studio opening with ${SKILLS_AT_THE_BAR} skills on CI: median ${median.toFixed(1)} ms`;
  if (platform !== "darwin") return { met: true, said: `${what}, no limit on this platform yet` };
  return {
    met: median < OPEN_GUARD_MEDIAN_MS,
    said: `${what} against the guard's ${OPEN_GUARD_MEDIAN_MS} ms, over ${samples.length} openings`,
  };
}

/** What a row's mark says (`MARK_SAID`, `src/skillsListing.ts`): a state, which is no reason. */
const STATE_WORDS = new Set(["needs review", "off", "left out", "hidden by the router", "lint"]);

/** **What is wrong with `said` as a skill's reason**, or nothing where it is one. */
export function reasonProblem(said: string): string | undefined {
  const line = said.trim();
  if (line === "") return "it says nothing";
  if (/\n/.test(line)) return `it runs to more than one line: ${JSON.stringify(said)}`;
  if (STATE_WORDS.has(line.toLowerCase()))
    return `it only names the state: ${JSON.stringify(said)}`;
  return undefined;
}

/** The harness a run of the suite is about. */
export type HarnessUnderTest = {
  kind: "claude" | "codex" | "opencode";
  /** The line Use skill… types for a skill called `name` that takes nothing. */
  typed: (name: string) => string;
};

const HARNESSES: readonly HarnessUnderTest[] = [
  { kind: "claude", typed: (name) => `/skills:${name}` },
  { kind: "codex", typed: (name) => `$${name}` },
  { kind: "opencode", typed: (name) => `/${name}` },
];

/**
 * **The harness `PURLIS_OUTCOME_HARNESS` names.** One run is one harness, since the app reads its
 * harness once (`wdio.state.conf.ts`), and a run that names none is refused: it would prove
 * nothing while passing.
 */
export function harnessUnderTest(named: string | undefined): HarnessUnderTest {
  const found = HARNESSES.find((one) => one.kind === named);
  if (found === undefined)
    throw new Error(
      `PURLIS_OUTCOME_HARNESS names the run's harness, one of ` +
        `${HARNESSES.map((one) => one.kind).join(", ")}, not ${String(named)}`,
    );
  return found;
}
