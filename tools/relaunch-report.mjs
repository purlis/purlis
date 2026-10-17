// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// What the relaunch test (`tools/relaunch.mjs`) reads and how it reports, apart from the script
// so that it can be tested: `relaunch.mjs` runs the app the moment it is imported.

import { ANSI, escapeData, escapeProperty, redacted } from "./e2e-annotations.mjs";

/** How much of what a check saw one annotation keeps: its head and its end. */
export const DETAIL_CHARS = 600;

/** GitHub's own limit on error annotations in one step. */
const PER_STEP = 10;

/**
 * The lines in `text` that are finished, a newline after each.
 *
 * The stand-in harness writes one line per time it is started, and the line runs to some 190 KB
 * (the settings the app hands a chat), written in more than one piece. A read between two pieces
 * sees a line's head without the `--resume` at its end; counted as a start, it ended a launch
 * before the line was whole, and the check on it failed (#2183). The newline is written last, so
 * a line that has one is whole.
 */
export function linesWritten(text) {
  const lines = text.split("\n");
  lines.pop();
  return lines.filter(Boolean);
}

/**
 * A record of the steps one launch went through and when each began, so a launch that failed
 * says which step it was in and which took the time. `now` is the clock, in ms.
 */
export function stepTimer(now = Date.now) {
  const steps = [];
  let stopped;
  return {
    /** Begins `name`, which ends the step before it; nothing, once the timer is stopped. */
    step(name) {
      if (stopped !== undefined) return;
      steps.push({ name, at: now() });
    },
    /** The step begun last, if any. */
    current() {
      return steps.at(-1)?.name;
    },
    /** Ends the last step: the launch is over, and a summary read later says how long it took. */
    stop() {
      stopped ??= now();
    },
    /** Each step and its seconds, the last one's counted to when the timer stopped, or to now. */
    summary() {
      if (steps.length === 0) return "no step started";
      const until = stopped ?? now();
      return steps
        .map(({ name, at }, index) => {
          const end = steps[index + 1]?.at ?? until;
          return `${name} ${((end - at) / 1000).toFixed(1)} s`;
        })
        .join(", ");
    },
  };
}

/** `detail` cut to DETAIL_CHARS around an ellipsis, its head and its end kept. */
function cut(detail) {
  if (detail.length <= DETAIL_CHARS) return detail;
  const half = Math.floor((DETAIL_CHARS - 1) / 2);
  return `${detail.slice(0, half)}…${detail.slice(detail.length - half)}`;
}

/**
 * The `::error` lines for the relaunch test's failed checks, read through the checks API like the
 * scenario tests' (`tools/e2e-annotations.mjs`): the job's log alone is not enough, and on macOS a
 * script that exits at once can cut its own last lines short (#2183).
 *
 * @param failures `{ launch, what, detail, steps }`: which launch, the check's words, what it saw,
 *   and the launch's steps with their times (`stepTimer().summary()`).
 */
export function annotations(failures) {
  const lines = failures.slice(0, PER_STEP).map(({ launch, what, detail, steps }) => {
    const title = ["relaunch", launch, what].filter(Boolean).join(" › ");
    // Redacted before it is cut: a secret the cut went through would be a fragment too short for
    // the shapes to know.
    const saw = cut(
      String(detail ?? "")
        .replace(ANSI, "")
        .split("\n")
        .map(redacted)
        .join("\n"),
    );
    const message = [saw, steps ? `steps: ${steps}` : ""].filter(Boolean).join("\n\n");
    return `::error title=${escapeProperty(title)}::${escapeData(message || "failed")}`;
  });
  const rest = failures.length - lines.length;
  if (rest > 0) {
    lines.push(
      `::notice title=+${rest} more failed relaunch checks::${escapeData(
        `${rest} more failed; ${failures.length} in all. The first ${lines.length} are annotated.`,
      )}`,
    );
  }
  return lines;
}
