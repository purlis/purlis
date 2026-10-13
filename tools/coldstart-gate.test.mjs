// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The Linux cold-start gate's decision (ADR 0086 row L6, as amended by V39). Run with
// `node --test tools/`.
import assert from "node:assert/strict";
import { test } from "node:test";

import { EventEmitter } from "node:events";

import {
  AGAINST_MAIN,
  NoMeasurement,
  coldStartAgainstMain,
  coldStartAnnotation,
  coldStartGate,
  firstFrame,
  launchSteps,
  noMeasurement,
} from "./coldstart-gate.mjs";

const gate = { limit: 2000, ceiling: 2500 };

test("one launch past the ceiling is reported and does not fail the gate", () => {
  // main's red run of 2026-10-02: a median of 1373 ms and one launch at 2687 ms.
  const result = coldStartGate({ launches: [1350, 1373, 2687, 1400, 1360], ...gate });
  assert.equal(result.met, true);
  assert.equal(result.median, 1373);
  assert.equal(result.worst, 2687);
  assert.deepEqual(result.ignored, [2687]);
});

test("two launches past the ceiling fail the gate", () => {
  const result = coldStartGate({ launches: [1350, 2537, 2687, 1400, 1360], ...gate });
  assert.equal(result.met, false);
});

test("a median past the limit fails the gate with no launch past the ceiling", () => {
  const result = coldStartGate({ launches: [2100, 2050, 2200, 1900, 2010], ...gate });
  assert.equal(result.met, false);
  assert.equal(result.median, 2050);
  assert.deepEqual(result.ignored, []);
});

test("five launches inside the limit meet the gate", () => {
  assert.equal(coldStartGate({ launches: [1350, 1373, 1560, 1400, 1360], ...gate }).met, true);
});

test("no launch at all does not meet the gate", () => {
  assert.equal(coldStartGate({ launches: [], ...gate }).met, false);
});

test("without a ceiling every launch is held to the limit, with no outlier ignored", () => {
  assert.equal(coldStartGate({ launches: [1350, 1373, 2001], limit: 2000 }).met, false);
  assert.equal(coldStartGate({ launches: [1350, 1373, 2000], limit: 2000 }).met, true);
});

test("a cold start's annotation says each launch's split, load and steps", () => {
  const line = coldStartAnnotation({
    name: "bare",
    limit: 2000,
    ceiling: 2500,
    warmUp: { ms: 3900, appMs: 1500, load: 1.5, steps: [] },
    launches: [1300, 1400, 1500, 1600, 1700].map((ms) => ({
      ms,
      appMs: ms - 200,
      load: 2.25,
      steps: [],
    })),
    traced: { ms: 1800, appMs: 1500, load: 2, steps: [{ ms: 40, step: "setup" }] },
  });
  assert.match(line, /^::notice title=Cold start%2C bare::/);
  assert.match(line, /median 1500 ms against a 2000 ms limit, worst 1700 ms against a 2500 ms ceiling: met/);
  assert.match(line, /%0Awarm-up, cold disk, not gated: 3900 ms; = 2400 ms before the app's clock/);
  assert.match(line, /%0Alaunch 1: 1300 ms; = 200 ms before the app's clock \+ 1100 ms in the app; load 2.25%0A/);
  assert.match(line, /%0Awith the launch log, not gated: 1800 ms; .*; steps: setup 40$/);
  assert.doesNotMatch(line, /\n/);
});

test("a missed cold start is an error annotation, and the gate's verdict is the gate's", () => {
  const launches = [2100, 2600, 2700, 1900, 2010].map((ms) => ({ ms }));
  const line = coldStartAnnotation({ name: "i3", limit: 2000, ceiling: 2500, launches });
  assert.match(line, /^::error title=Cold start%2C i3::median 2100 ms .*: MISSED/);
  assert.equal(
    coldStartGate({ launches: launches.map((one) => one.ms), limit: 2000, ceiling: 2500 }).met,
    false,
  );
});

test("the launch steps are read from what the app printed, and nothing else", () => {
  const output = "noise\ncharter-launch    12 ms  run() entered\nother\ncharter-launch   640 ms  the window is built\n";
  assert.deepEqual(launchSteps(output), [
    { ms: 12, step: "run() entered" },
    { ms: 640, step: "the window is built" },
  ]);
  assert.deepEqual(launchSteps("nothing here"), []);
});

// **Against main, in the same job** (#1866). The launches below are what the Ubuntu runners
// measured across train 84 and 85's runs, none of which changed the launch, read off each job's
// annotation: within a job five launches sit within a few percent of each other, apart from a
// slow one now and then, while job to job the median moved from 874 to 2537 ms.
const runners = {
  // The bare case's red run (job 114885524458): a median of 2537 ms with the launch unchanged.
  slowest: [2539, 2138, 2550, 2537, 2386],
  // i3-nobus, job 114992049755: two launches in a row 42% and 68% slower than the rest.
  spiky: [1193, 1173, 1697, 2000, 1165],
  // bare, job 114986900676: the tightest.
  fastest: [1396, 1333, 1373, 1336, 1348],
  // i3, job 114972575058: 1370 to 2081 ms in one job.
  widest: [1370, 2081, 1860, 1633, 1394],
};

/** Rounds of main's launches against `head`'s, the head's taken one launch later in the job. */
function rounds(base, head = base) {
  return base.map((ms, n) => ({ base: ms, head: head[(n + 1) % head.length] }));
}

test("main against itself in one job passes, on every runner, the slowest included", () => {
  for (const [name, launches] of Object.entries(runners)) {
    const passes = [rounds(launches)];
    const verdict = coldStartAgainstMain({ passes });
    assert.equal(verdict.verdict, "pass", `${name}: ${JSON.stringify(verdict)}`);
  }
  // The same slowest runner's launches miss the absolute limit the gate held before.
  assert.equal(coldStartGate({ launches: runners.slowest, limit: 2000, ceiling: 2500 }).met, false);
});

test("a change 30% slower than main is regressed, on every runner, once a second pass agrees", () => {
  for (const [name, launches] of Object.entries(runners)) {
    const slower = launches.map((ms) => ms * 1.3);
    const first = coldStartAgainstMain({ passes: [rounds(launches, slower)] });
    assert.equal(first.verdict, "confirm", `${name}: ${JSON.stringify(first)}`);
    const both = coldStartAgainstMain({ passes: [rounds(launches, slower), rounds(launches, slower)] });
    assert.equal(both.verdict, "regressed", `${name}: ${JSON.stringify(both)}`);
  }
});

test("a planted delay of 30% of main's median is regressed on every runner, the slowest included", () => {
  for (const [name, launches] of Object.entries(runners)) {
    const sorted = [...launches].sort((a, b) => a - b);
    const delay = 0.3 * sorted[2];
    const slower = launches.map((ms) => ms + delay);
    const verdict = coldStartAgainstMain({ passes: [rounds(launches, slower), rounds(launches, slower)] });
    assert.equal(verdict.verdict, "regressed", `${name}: ${JSON.stringify(verdict)}`);
  }
});

test("one pass past the line that a second does not confirm passes, said as unconfirmed", () => {
  const slower = runners.fastest.map((ms) => ms * 1.3);
  const verdict = coldStartAgainstMain({
    passes: [rounds(runners.fastest, slower), rounds(runners.fastest)],
  });
  assert.equal(verdict.verdict, "pass");
  assert.equal(verdict.unconfirmed, true);
});

test("the line is a ratio of 1.15, the median of the rounds' ratios", () => {
  assert.equal(AGAINST_MAIN, 1.15);
  const at = (ratio) =>
    coldStartAgainstMain({ passes: [[1000, 1000, 1000].map((base) => ({ base, head: base * ratio }))] });
  assert.equal(at(1.15).verdict, "pass");
  assert.equal(at(1.16).verdict, "confirm");
  assert.equal(at(1.16).ratio, 1.16);
  // One slow round of three is not the median.
  const one = coldStartAgainstMain({
    passes: [
      [
        { base: 1000, head: 2000 },
        { base: 1000, head: 1000 },
        { base: 1000, head: 1010 },
      ],
    ],
  });
  assert.equal(one.verdict, "pass");
});

test("no rounds at all is no baseline", () => {
  assert.deepEqual(coldStartAgainstMain({ passes: [] }), { verdict: "no baseline" });
  assert.deepEqual(coldStartAgainstMain({ passes: [[]] }), { verdict: "no baseline" });
});

test("against main, the annotation names the ratio and each round, and the limit is reported", () => {
  const launches = (list) => list.map((ms) => ({ ms }));
  const passes = [rounds(runners.slowest)];
  const line = coldStartAnnotation({
    name: "bare",
    limit: 2000,
    ceiling: 2500,
    launches: launches(runners.slowest),
    againstMain: { passes, ...coldStartAgainstMain({ passes }) },
  });
  assert.match(line, /^::notice title=Cold start%2C bare::against main in this job: /);
  assert.match(line, /ratio 0\.99\d? against a 1\.15 line: pass/);
  assert.match(line, /%0Around 1: main 2539 ms, this change 2138 ms/);
  assert.match(line, /median 2537 ms against a 2000 ms limit.*: missed, reported, not gated/);
  assert.doesNotMatch(line, /\n/);
});

test("against main, a confirmed regression is an error annotation", () => {
  const slower = runners.fastest.map((ms) => ms * 1.3);
  const passes = [rounds(runners.fastest, slower), rounds(runners.fastest, slower)];
  const line = coldStartAnnotation({
    name: "i3",
    limit: 2000,
    ceiling: 2500,
    launches: slower.map((ms) => ({ ms })),
    againstMain: { passes, ...coldStartAgainstMain({ passes }) },
  });
  assert.match(line, /^::error title=Cold start%2C i3::against main in this job: .*: REGRESSED/);
});

// A launched app, as much of one as `firstFrame` reads.
function launched() {
  const app = new EventEmitter();
  app.stdout = new EventEmitter();
  app.stderr = new EventEmitter();
  return app;
}

test("a launch's first frame is the app's own line, whatever came before it", async () => {
  const app = launched();
  const heard = [];
  const frame = firstFrame(app, { heard: (chunk, stream) => heard.push([stream, `${chunk}`]) });
  app.stderr.emit("data", "a slow portal\n");
  app.stdout.emit("data", "charter-bench first-");
  app.stdout.emit("data", "frame 412\n");
  assert.equal((await frame).appMs, 412);
  assert.deepEqual(heard[0], ["stderr", "a slow portal\n"]);
});

test("a window that never appeared is no measurement, and not the change's to fail for", async () => {
  const app = launched();
  const frame = firstFrame(app, { timeoutMs: 20 });
  app.stderr.emit("data", "Gdk: the display went away\n");
  await assert.rejects(frame, (error) => {
    assert.ok(error instanceof NoMeasurement);
    assert.match(error.message, /this change's build drew no first frame within 0\.02 s/);
    assert.match(error.detail, /the display went away/);
    assert.equal(error.fails, false);
    return true;
  });
});

test("a build that left before its first frame fails when it is the change's, not when it is main's", async () => {
  const change = launched();
  const ofChange = firstFrame(change);
  change.stderr.emit("data", "one\n".repeat(30) + "the last line\n");
  change.emit("exit", 1, null);
  await assert.rejects(ofChange, (error) => {
    assert.match(error.message, /this change's build exited \(1\) before its first frame/);
    assert.equal(error.fails, true);
    // The last of what it said, not all of it.
    assert.equal(error.detail.split("\n").length, 12);
    assert.match(error.detail, /the last line$/);
    return true;
  });
  const main = launched();
  const ofMain = firstFrame(main, { ofMain: true });
  main.emit("exit", null, "SIGSEGV");
  await assert.rejects(ofMain, (error) => {
    assert.match(error.message, /main's build exited \(SIGSEGV\) before its first frame/);
    assert.equal(error.fails, false);
    return true;
  });
});

test("a binary that could not be started is an error of the run, not a missing measurement", async () => {
  const app = launched();
  const frame = firstFrame(app);
  app.emit("error", new Error("spawn ENOENT"));
  await assert.rejects(frame, (error) => !(error instanceof NoMeasurement) && /could not be started/.test(error.message));
});

test("no measurement is a warning that passes only where the gate reports and the cause is the runner's", () => {
  const cause = "main's build drew no first frame within 30 s: its window never appeared";
  const reported = noMeasurement({ name: "i3", cause, detail: "line one\nline two", evidence: true });
  assert.equal(reported.failed, false);
  assert.equal(
    reported.annotation,
    "::warning title=Cold start%2C i3%3A no measurement::" +
      `${cause}; reported, and the run does not fail for it (evidence only)%0Aline one%0Aline two`,
  );
  assert.match(reported.summary, /^### Cold start, i3: no measurement\n\nmain's build drew/);

  const gated = noMeasurement({ name: "bare", cause, evidence: false });
  assert.equal(gated.failed, true);
  assert.equal(gated.annotation, `::error title=Cold start%2C bare%3A no measurement::${cause}`);

  const own = noMeasurement({ name: "i3", cause: "this change's build exited (1)", evidence: true, fails: true });
  assert.equal(own.failed, true);
  assert.match(own.annotation, /^::error /);
});
