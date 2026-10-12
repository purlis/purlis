// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Whether a cold-start run meets its limit (docs/spec.md row L6, ADR 0086 as amended by V39 and
// for #1866).
//
// **Against main, in the same job** (CI's Linux gate on a pull request, #1866): main's build and
// the change's, launched in turn on one runner, so the runner's speed is in both halves of every
// ratio and cancels out (`coldStartAgainstMain`). The absolute limit below is then reported, not
// gated: on unchanged code its median moved from 874 to 2537 ms between runners.
//
// **The absolute limit** (`coldStartGate`), where there is no main to measure against: a push
// to main, or a pull request whose main did not build.
//
// With a ceiling (CI's Linux gate): the median of the launches must be within the limit, and at
// most one launch may go past the ceiling. That one is an outlier, reported and not gated: one
// slow launch on a shared runner is not a regression (main went red twice on 2026-10-02 from a
// single launch each, 2687 ms and 2537 ms, with medians of 1373 and 1560 ms). Two or more past
// the ceiling fail.
//
// Without a ceiling: every launch is held to the limit.
//
// `launches` is in ms; the result's `over` is every launch past the ceiling (or the limit), and
// `ignored` the one outlier the gate let through, if any.
export function coldStartGate({ launches, limit, ceiling }) {
  const median = medianOf(launches);
  const worst = Math.max(0, ...launches);
  if (ceiling === undefined) {
    const over = launches.filter((ms) => ms > limit);
    return { met: launches.length > 0 && over.length === 0, median, worst, over, ignored: [] };
  }
  const over = launches.filter((ms) => ms > ceiling);
  const met = launches.length > 0 && median <= limit && over.length <= 1;
  return { met, median, worst, over, ignored: met ? over : [] };
}

function medianOf(values) {
  const sorted = [...values].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
}

/** The ratio past which a change's cold start is slower than main's (#1866). */
export const AGAINST_MAIN = 1.15;

// **Whether a change's cold start is slower than main's, measured in the same job** (#1866).
//
// A pass is rounds `{ base, head }` in ms, each one launch of main's build and one of the
// change's, in alternating order. Its ratio is the median of its rounds' ratios, the change over
// main, so one slow launch of either is never the verdict. A pass past `threshold` asks for a
// second (`confirm`); the change has REGRESSED only when every pass is past it. A first pass
// past it that a second does not confirm passes, said as `unconfirmed`. No rounds at all is
// `no baseline`, and the caller holds the absolute limit instead.
//
// Why 1.15 and two passes: on the launches the Ubuntu runners recorded for builds that did not change the launch,
// resampled as main against itself, no pass in 20,000 went past it, and a change 30% slower
// (or 30% of main's median added to each launch) was confirmed in over 99% of them.
export function coldStartAgainstMain({ passes, threshold = AGAINST_MAIN }) {
  const judged = passes
    .filter((rounds) => rounds.length > 0)
    .map((rounds) => {
      const ratio = medianOf(rounds.map(({ base, head }) => head / base));
      return { ratio: Math.round(ratio * 1000) / 1000, past: ratio > threshold };
    });
  if (judged.length === 0) return { verdict: "no baseline" };
  const last = judged.at(-1);
  if (!last.past)
    return { verdict: "pass", ratio: last.ratio, unconfirmed: judged.slice(0, -1).some((one) => one.past) };
  if (judged.length >= 2 && judged.every((one) => one.past)) return { verdict: "regressed", ratio: last.ratio };
  return { verdict: "confirm", ratio: last.ratio };
}

// **Where a cold start's time went, as one CI annotation** (a diagnostic, main run 38203858940).
// The gate's verdict alone printed to the log, so a red run showed only "exit code 1", and the
// log is not what the program reads. This puts on the run's summary, for each held launch: its
// time, the part before the app's own clock starts (exec, the dynamic loader, a cold page
// cache), the part the app counts, the runner's one-minute load when it was launched, and the
// app's launch steps (`PURLIS_LAUNCH_LOG`) — enough to tell a slow runner from a slow app.
//
// `launches` are `{ ms, appMs, load, steps: [{ ms, step }] }`; `warmUp` is the discarded launch
// and `traced` the one launched after the held ones with the launch log on. Neither is gated.
// A missed gate is an `error`, a met one a `notice`. It says, it decides nothing.
//
// `againstMain`, when the job measured main's build too, is `{ passes, verdict, ratio }`: its
// verdict is the gate, each round of the last pass is listed, and the absolute limit is reported,
// not gated. `evidence` makes a miss a `warning`: a run that only records.
export function coldStartAnnotation({ name, launches, warmUp, traced, limit, ceiling, againstMain, evidence }) {
  const absolute = coldStartGate({
    launches: launches.map((one) => one.ms),
    limit,
    ceiling,
  });
  const { median, worst, ignored } = absolute;
  const relative = againstMain && againstMain.verdict !== "no baseline";
  const met = relative ? againstMain.verdict !== "regressed" : absolute.met;
  const ms = (value) => `${Math.round(value)} ms`;
  const one = (launch) => {
    const parts = [ms(launch.ms)];
    if (Number.isFinite(launch.appMs))
      parts.push(`= ${ms(launch.ms - launch.appMs)} before the app's clock + ${ms(launch.appMs)} in the app`);
    if (Number.isFinite(launch.load)) parts.push(`load ${launch.load.toFixed(2)}`);
    if (launch.steps?.length > 0)
      parts.push(`steps: ${launch.steps.map(({ ms: at, step }) => `${step} ${at}`).join(", ")}`);
    return parts.join("; ");
  };
  const limitLine =
    `median ${ms(median)} against a ${limit} ms limit, worst ${ms(worst)}` +
    (ceiling === undefined ? "" : ` against a ${ceiling} ms ceiling`) +
    (relative
      ? `: ${absolute.met ? "met" : "missed"}, reported, not gated`
      : `: ${absolute.met ? "met" : "MISSED"}` +
        (ignored.length > 0 ? ` (one launch past the ceiling let through)` : ""));
  const mainLines = relative
    ? [
        `against main in this job: ${againstMain.passes.length} pass(es), ratio ${againstMain.ratio} ` +
          `against a ${AGAINST_MAIN} line: ${againstMain.verdict === "regressed" ? "REGRESSED" : againstMain.verdict}` +
          (againstMain.unconfirmed ? " (one pass past the line, not confirmed)" : ""),
        ...againstMain.passes
          .at(-1)
          .map((round, n) => `round ${n + 1}: main ${ms(round.base)}, this change ${ms(round.head)}`),
      ]
    : [];
  const lines = [
    ...mainLines,
    limitLine,
    ...(warmUp ? [`warm-up, cold disk, not gated: ${one(warmUp)}`] : []),
    ...launches.map((launch, n) => `launch ${n + 1}: ${one(launch)}`),
    ...(traced ? [`with the launch log, not gated: ${one(traced)}`] : []),
  ];
  const data = (text) => text.replace(/%/g, "%25").replace(/\r/g, "%0D").replace(/\n/g, "%0A");
  const property = (text) => data(text).replace(/:/g, "%3A").replace(/,/g, "%2C");
  const level = met ? "notice" : evidence ? "warning" : "error";
  return `::${level} title=${property(`Cold start, ${name}`)}::${data(lines.join("\n"))}`;
}

/** The steps a launch printed with `PURLIS_LAUNCH_LOG` (`reached` in the app's `lib.rs`). */
export function launchSteps(output) {
  return [...output.matchAll(/charter-launch\s+(\d+) ms {2}(.+)/g)].map((found) => ({
    ms: Number(found[1]),
    step: found[2].trim(),
  }));
}
