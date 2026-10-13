// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// What `tools/bench.mjs`'s `restore` case reads and says: a launch that puts N chats back, by
// when its window could be used and when its chats had started (ADR 0086 row L8;
// `app/src-tauri/src/restoring.rs`). Evidence only: it is reported and fails nothing.

/** The marks the app prints under `PURLIS_BENCH_LOG`, as `charter-bench <mark> <ms>`. */
export const RESTORE_MARKS = ["first-frame", "usable", "first-chat", "all-chats"];

/** The sizes the case launches with: one chat, a working day's, and the hot target's. */
export const RESTORE_SIZES = [1, 10, 40];

/**
 * How much longer the window may take to be usable with the most chats than with one, before
 * the case says its wait grows with them: half a second, far above a launch's own spread and
 * far below what forty starts cost.
 */
export const RESTORE_GROWS_PAST_MS = 500;

/** The marks in what the app printed so far, by name: the first of each, in ms since its start. */
export function restoreMarks(said) {
  const marks = {};
  for (const [, name, ms] of said.matchAll(/charter-bench ([a-z-]+) (\d+)/g)) {
    if (RESTORE_MARKS.includes(name) && marks[name] === undefined) marks[name] = Number(ms);
  }
  return marks;
}

/** Whether a launch has said every mark, so it can be ended. */
export function restoreIsWhole(marks) {
  return RESTORE_MARKS.every((name) => marks[name] !== undefined);
}

const median = (values) => {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted.length === 0 ? undefined : sorted[Math.floor(sorted.length / 2)];
};

/**
 * The case's verdict from its launches, `{ [chats]: [marks, ...] }`: each size's median of each
 * mark, and whether the window's wait grows with the chats put back. A launch that never said
 * `usable` (a build from before the case) leaves that size with no number, and no verdict.
 */
export function restoreVerdict(launches) {
  const rows = Object.entries(launches)
    .map(([chats, samples]) => ({
      chats: Number(chats),
      launches: samples.length,
      ...Object.fromEntries(
        RESTORE_MARKS.map((name) => [
          name,
          median(samples.map((one) => one[name]).filter((ms) => ms !== undefined)),
        ]),
      ),
    }))
    .sort((a, b) => a.chats - b.chats);
  const usable = rows.map((row) => row.usable);
  const measured = rows.length > 1 && usable.every((ms) => ms !== undefined);
  const grows = measured ? usable.at(-1) - usable[0] : undefined;
  return {
    rows,
    usableGrowsByMs: grows,
    verdict: !measured
      ? "not measured"
      : grows > RESTORE_GROWS_PAST_MS
        ? "the window waits on the chats"
        : "the window does not wait on the chats",
  };
}

/** The verdict as a Markdown table, for the log and the job's summary. */
export function restoreTable({ rows, usableGrowsByMs, verdict }) {
  const ms = (value) => (value === undefined ? "—" : `${Math.round(value)} ms`);
  const lines = [
    "| chats put back | first frame | window usable | first chat started | every chat started |",
    "| --- | --- | --- | --- | --- |",
    ...rows.map(
      (row) =>
        `| ${row.chats} | ${ms(row["first-frame"])} | ${ms(row.usable)} | ${ms(row["first-chat"])} | ${ms(row["all-chats"])} |`,
    ),
    "",
    usableGrowsByMs === undefined
      ? `Verdict: ${verdict}.`
      : `Verdict: ${verdict} (usable ${Math.round(usableGrowsByMs)} ms later with the most chats than with one; the line is ${RESTORE_GROWS_PAST_MS} ms). Evidence only.`,
  ];
  return lines.join("\n");
}
