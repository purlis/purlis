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
// to main, a nightly, or a pull request whose main did not build. In CI each of those reports it
// and gates nothing: a push to main and a nightly are evidence (the pull request held the same
// code to main already), and a pull request with no main build has nothing that cancels the
// runner's speed, which missed the limit in 39 of 1,572 cases on unchanged launch code (#2388,
// ADR 0086 as amended 2026-10-17). It gates where it is run without `--evidence`, by hand.
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
// A pass is rounds `{ base, head }`, each one launch of main's build and one of the change's,
// in alternating order, each a launch as the bench keeps it (`{ ms, appMs, use, ... }`; only `ms`
// is judged). Its ratio is the median of its rounds' ratios, the change over
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
      const ratio = medianOf(rounds.map(({ base, head }) => head.ms / base.ms));
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
//
// `noBaseline`, on a pull request whose job could not build main's app (#2388), is why there is
// nothing to hold the change to. Nothing is gated, the limit is reported, and the annotation is
// a `warning` even when the limit is met, so the run says on its face that it went ungated.
export function coldStartAnnotation({
  name,
  launches,
  warmUp,
  traced,
  limit,
  ceiling,
  againstMain,
  evidence,
  noBaseline,
}) {
  const absolute = coldStartGate({
    launches: launches.map((one) => one.ms),
    limit,
    ceiling,
  });
  const { median, worst, ignored } = absolute;
  const relative = againstMain && againstMain.verdict !== "no baseline";
  // No main to hold the change to on a pull request: said first, and nothing is gated.
  const ungated = Boolean(noBaseline) && !relative;
  const reported = relative || ungated;
  const met = relative ? againstMain.verdict !== "regressed" : absolute.met;
  const ms = (value) => `${Math.round(value)} ms`;
  const split = (launch) => `${ms(launch.ms - launch.appMs)} before the app's clock + ${ms(launch.appMs)} in the app`;
  const use = ({ cpuMs, majorFaults, processes }) =>
    `CPU ${ms(cpuMs)} in ${processes} processes, ${majorFaults} major faults`;
  // A round's launch: its time, then its split and what its process tree used where it has them.
  const inRound = (launch) => {
    const parts = [...(Number.isFinite(launch.appMs) ? [split(launch)] : []), ...(launch.use ? [use(launch.use)] : [])];
    return parts.length > 0 ? `${ms(launch.ms)} (${parts.join("; ")})` : ms(launch.ms);
  };
  const one = (launch) => {
    const parts = [ms(launch.ms)];
    if (Number.isFinite(launch.appMs)) parts.push(`= ${split(launch)}`);
    if (launch.use) parts.push(use(launch.use));
    if (Number.isFinite(launch.load)) parts.push(`load ${launch.load.toFixed(2)}`);
    if (launch.steps?.length > 0)
      parts.push(`steps: ${launch.steps.map(({ ms: at, step }) => `${step} ${at}`).join(", ")}`);
    return parts.join("; ");
  };
  const limitLine =
    `median ${ms(median)} against a ${limit} ms limit, worst ${ms(worst)}` +
    (ceiling === undefined ? "" : ` against a ${ceiling} ms ceiling`) +
    (reported
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
          .map(
            (round, n) =>
              `round ${n + 1}: main ${inRound(round.base)}, this change ${inRound(round.head)}`,
          ),
      ]
    : [];
  const lines = [
    ...(ungated ? [`no baseline: ${noBaseline}, so nothing is gated`] : []),
    ...mainLines,
    limitLine,
    ...(warmUp ? [`warm-up, cold disk, not gated: ${one(warmUp)}`] : []),
    ...launches.map((launch, n) => `launch ${n + 1}: ${one(launch)}`),
    ...(traced ? [`with the launch log, not gated: ${one(traced)}`] : []),
  ];
  const level = ungated ? "warning" : met ? "notice" : evidence ? "warning" : "error";
  return annotation(level, `Cold start, ${name}`, lines.join("\n"));
}

/** The check whose result on main's commit says whether main's app builds there. */
export const MAINS_BUILD_CHECK = "app builds (ubuntu-24.04)";

// **What a pull request does when main's app did not build in its job** (#2388, ADR 0086 as
// amended 2026-10-17). main is built in the change's own job, in the environment the change set
// up, so the change can break main's build there alone: a toolchain bump, a dropped package, a
// build script that writes the job's environment or the shared target directory. Going ungated on
// that would let a change switch its own gate off. So it is no baseline only when main's own
// `MAINS_BUILD_CHECK` on that commit is not green either: failed, cancelled, still running or
// absent. main green there fails the run, and so does a check that could not be read, since then
// nothing says main is broken.
//
// `runs` are the GitHub check runs of that name on main's commit `sha` (`status`, `conclusion`,
// `html_url`), `error` what reading them said when it failed. The result's `noBaseline` is the
// reason the cold start is told (`coldStartAnnotation`), `annotation` the step's own line.
export function withoutMainsBuild({ sha, runs = [], error }) {
  const at = sha.slice(0, 12);
  const failing = (text) => ({
    fails: true,
    annotation: annotation("error", "Cold start: main's app did not build in this job", text),
  });
  if (error !== undefined)
    return failing(
      `main's app did not build in this job, and main's own ${MAINS_BUILD_CHECK} on ${at} could not be read: ${error}`,
    );
  const green = runs.find((run) => run.status === "completed" && run.conclusion === "success");
  if (green)
    return failing(
      `main's app built on main (${at}: ${MAINS_BUILD_CHECK} passed, ${green.html_url}) but not in this job, ` +
        "so this change broke main's build here; with no baseline the cold start would go ungated, so the run fails",
    );
  const state =
    runs.length === 0 ? `no ${MAINS_BUILD_CHECK} check` : runs.map((run) => run.conclusion ?? run.status).join(", ");
  const noBaseline = `main's app did not build in this job, nor on main (${at}: ${state})`;
  return {
    fails: false,
    noBaseline,
    annotation: annotation(
      "warning",
      "Cold start: no baseline",
      `${noBaseline}, so the cold start is reported, not gated`,
    ),
  };
}

// **What a launch's processes used by its first frame** (#1866): the CPU time and the major page
// faults of the launched process and every process below it (WebKit's do most of a first frame's
// work), read from `/proc/<pid>/stat` on Linux. Two of 524 pull request runs held one build
// steadily slower than the other through a whole job on unchanged launch code. The round lines
// carry this for both builds, so the next one says whether that build read its binary off the
// disk on every launch (major faults), ran more code (CPU), or waited on a busy runner (neither).
//
// `stats` are the text of each process's `stat` file, `root` the launched pid and `ticksPerSecond`
// the kernel's clock (`getconf CLK_TCK`). `undefined` when the launched process is not among them.
export function processTreeUse(stats, root, ticksPerSecond) {
  const processes = new Map();
  for (const text of stats) {
    // The name is in parentheses and may hold either, so the fields start after the last `)`.
    const close = text.lastIndexOf(")");
    if (close < 0) continue;
    const pid = Number(text.slice(0, text.indexOf(" ")));
    // fields[0] is field 3 (state): ppid is field 4, majflt 12, utime 14 and stime 15.
    const fields = text.slice(close + 1).trim().split(/\s+/);
    processes.set(pid, {
      ppid: Number(fields[1]),
      majorFaults: Number(fields[9]),
      ticks: Number(fields[11]) + Number(fields[12]),
    });
  }
  if (!processes.has(root)) return undefined;
  const tree = new Set([root]);
  for (let grew = true; grew; ) {
    grew = false;
    for (const [pid, { ppid }] of processes)
      if (!tree.has(pid) && tree.has(ppid)) {
        tree.add(pid);
        grew = true;
      }
  }
  let ticks = 0;
  let majorFaults = 0;
  for (const pid of tree) {
    ticks += processes.get(pid).ticks;
    majorFaults += processes.get(pid).majorFaults;
  }
  return { cpuMs: Math.round((ticks * 1000) / ticksPerSecond), majorFaults, processes: tree.size };
}

/** One workflow command: an annotation on the run, with its title and text escaped. */
export function annotation(level, title, text) {
  const data = (value) => value.replace(/%/g, "%25").replace(/\r/g, "%0D").replace(/\n/g, "%0A");
  const property = (value) => data(value).replace(/:/g, "%3A").replace(/,/g, "%2C");
  return `::${level} title=${property(title)}::${data(text)}`;
}

/** How long a launch may take to its first frame before the round is given up. */
export const FIRST_FRAME_MS = 30_000;

// **A cold start that could not be measured** (#1876): the window never drew, or a build left
// before it did. It is not a slow launch, so no gate has a number to judge. `cause` names what
// happened and to which build; `detail` is the last of what that build said on stderr.
//
// `fails` is the one case that is the change's own: its build left before its first frame. That
// is an app that does not start, and it fails the run even where the gate only reports. main's
// build doing the same, or either build's window never appearing, is the runner's.
export class NoMeasurement extends Error {
  constructor(cause, { detail = "", fails = false } = {}) {
    super(cause);
    this.name = "NoMeasurement";
    this.detail = detail;
    this.fails = fails;
  }
}

/**
 * A launched app's first frame: `{ at, appMs }` when it prints `charter-bench first-frame`.
 * It rejects with `NoMeasurement` when the app leaves first or `timeoutMs` passes, and with a
 * plain error when the binary could not be started at all. `heard` gets every chunk it reads.
 */
export function firstFrame(app, { timeoutMs = FIRST_FRAME_MS, ofMain = false, heard = () => {} } = {}) {
  const build = ofMain ? "main's build" : "this change's build";
  let seen = "";
  let errors = "";
  const last = () => errors.trim().split("\n").slice(-12).join("\n");
  return new Promise((resolve, reject) => {
    const giveUp = setTimeout(
      () =>
        reject(
          new NoMeasurement(
            `${build} drew no first frame within ${timeoutMs / 1000} s: its window never appeared`,
            { detail: last() },
          ),
        ),
      timeoutMs,
    );
    app.stdout.on("data", (chunk) => {
      seen += chunk;
      heard(chunk, "stdout");
      const found = seen.match(/charter-bench first-frame (\d+)/);
      if (found) {
        clearTimeout(giveUp);
        resolve({ at: performance.now(), appMs: Number(found[1]) });
      }
    });
    app.stderr.on("data", (chunk) => {
      errors += chunk;
      heard(chunk, "stderr");
    });
    app.once("error", (error) => {
      clearTimeout(giveUp);
      reject(new Error(`${build} could not be started: ${error.message}`));
    });
    app.once("exit", (code, signal) => {
      clearTimeout(giveUp);
      // Its last words may still be in the pipe: they are the cause, so they are waited for.
      setTimeout(
        () =>
          reject(
            new NoMeasurement(`${build} exited (${code ?? signal}) before its first frame`, {
              detail: last(),
              fails: !ofMain,
            }),
          ),
        50,
      );
    });
  });
}

// **What a run says when it has no measurement** (#1876): one annotation that names the cause and
// the case, so the next one is read without the log. A `warning`, and the run passes, where the
// gate only reports (`evidence`) and the cause is not the change's own (`fails`); an `error`
// otherwise. `failed` is what the caller exits with.
export function noMeasurement({ name, cause, detail = "", evidence = false, fails = false }) {
  const failed = fails || !evidence;
  const lines = [
    failed ? cause : `${cause}; reported, and the run does not fail for it (evidence only)`,
    ...(detail ? [detail] : []),
  ];
  return {
    failed,
    title: `Cold start, ${name}: no measurement`,
    annotation: annotation(failed ? "error" : "warning", `Cold start, ${name}: no measurement`, lines.join("\n")),
    summary: `### Cold start, ${name}: no measurement\n\n${cause}\n`,
  };
}

/** The steps a launch printed with `PURLIS_LAUNCH_LOG` (`reached` in the app's `lib.rs`). */
export function launchSteps(output) {
  return [...output.matchAll(/charter-launch\s+(\d+) ms {2}(.+)/g)].map((found) => ({
    ms: Number(found[1]),
    step: found[2].trim(),
  }));
}
