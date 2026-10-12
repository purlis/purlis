// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Whether a cold-start run meets its limit (docs/spec.md row L6, ADR 0086 as amended by V39).
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
  const sorted = [...launches].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  const median = sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
  const worst = sorted.at(-1) ?? 0;
  if (ceiling === undefined) {
    const over = launches.filter((ms) => ms > limit);
    return { met: launches.length > 0 && over.length === 0, median, worst, over, ignored: [] };
  }
  const over = launches.filter((ms) => ms > ceiling);
  const met = launches.length > 0 && median <= limit && over.length <= 1;
  return { met, median, worst, over, ignored: met ? over : [] };
}

// **Where a cold start's time went, as one CI annotation** (a diagnostic, main run 38203858940).
// The gate's verdict alone printed to the log, so a red run showed only "exit code 1", and the
// log is not what the program reads. This puts on the run's summary, for each held launch: its
// time, the part before the app's own clock starts (exec, the dynamic loader, a cold page
// cache), the part the app counts, the runner's one-minute load when it was launched, and the
// app's launch steps (`PURLIS_LAUNCH_LOG`) — enough to tell a slow runner from a slow app.
//
// `launches` are `{ ms, appMs, load, steps: [{ ms, step }] }`; `warmUp` is the discarded launch.
// A missed gate is an `error`, a met one a `notice`. It says, it decides nothing.
export function coldStartAnnotation({ name, launches, warmUp, limit, ceiling }) {
  const { met, median, worst, ignored } = coldStartGate({
    launches: launches.map((one) => one.ms),
    limit,
    ceiling,
  });
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
  const lines = [
    `median ${ms(median)} against a ${limit} ms limit, worst ${ms(worst)}` +
      (ceiling === undefined ? "" : ` against a ${ceiling} ms ceiling`) +
      `: ${met ? "met" : "MISSED"}` +
      (ignored.length > 0 ? ` (one launch past the ceiling let through)` : ""),
    ...(warmUp ? [`warm-up, cold disk, not gated: ${one(warmUp)}`] : []),
    ...launches.map((launch, n) => `launch ${n + 1}: ${one(launch)}`),
  ];
  const data = (text) => text.replace(/%/g, "%25").replace(/\r/g, "%0D").replace(/\n/g, "%0A");
  const property = (text) => data(text).replace(/:/g, "%3A").replace(/,/g, "%2C");
  return `::${met ? "notice" : "error"} title=${property(`Cold start, ${name}`)}::${data(lines.join("\n"))}`;
}

/** The steps a launch printed with `PURLIS_LAUNCH_LOG` (`reached` in the app's `lib.rs`). */
export function launchSteps(output) {
  return [...output.matchAll(/charter-launch\s+(\d+) ms {2}(.+)/g)].map((found) => ({
    ms: Number(found[1]),
    step: found[2].trim(),
  }));
}
