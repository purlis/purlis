// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The Linux cold-start gate's decision (ADR 0086 row L6, as amended by V39). Run with
// `node --test tools/`.
import assert from "node:assert/strict";
import { test } from "node:test";

import { coldStartAnnotation, coldStartGate, launchSteps } from "./coldstart-gate.mjs";

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
      steps: [{ ms: 40, step: "setup" }],
    })),
  });
  assert.match(line, /^::notice title=Cold start%2C bare::/);
  assert.match(line, /median 1500 ms against a 2000 ms limit, worst 1700 ms against a 2500 ms ceiling: met/);
  assert.match(line, /%0Awarm-up, cold disk, not gated: 3900 ms; = 2400 ms before the app's clock/);
  assert.match(line, /%0Alaunch 1: 1300 ms; = 200 ms before the app's clock \+ 1100 ms in the app; load 2.25; steps: setup 40/);
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
