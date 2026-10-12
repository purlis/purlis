// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The latency gate's decision (SC-16, ADR 0086's CI relative rows). Run with
// `node --test tools/`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { GATED, latencyGate } from "./latency-gate.mjs";

/** A row the gate holds a change to (#932): one of the two bursts. */
const BURST = "2 MB burst, asked to drawn";
/** The row that stays evidence: main's own five records spread by far more than 20%. */
const KEYSTROKE = "keystroke under ten flooding panes";

/** Five interleaved rounds, each a median of one run's samples, main's and the change's. */
const rounds = (base, head) => base.map((b, n) => ({ base: b, head: head[n] }));

test("a change that measures the same as main passes", () => {
  const pass = rounds([2.0, 2.1, 1.9, 2.0, 2.05], [2.02, 2.08, 1.95, 2.01, 2.0]);
  assert.equal(latencyGate({ name: BURST, passes: [pass] }).verdict, "pass");
});

test("a 30% regression in every round asks for a second pass before it fails", () => {
  const slow = rounds([2.0, 2.1, 1.9, 2.0, 2.05], [2.6, 2.73, 2.47, 2.6, 2.67]);
  const first = latencyGate({ name: BURST, passes: [slow] });
  assert.equal(first.verdict, "confirm");
  assert.equal(first.ratio, 1.3);
});

test("a regression the second pass confirms fails", () => {
  const slow = rounds([2.0, 2.1, 1.9, 2.0, 2.05], [2.6, 2.73, 2.47, 2.6, 2.67]);
  assert.equal(latencyGate({ name: BURST, passes: [slow, slow] }).verdict, "regressed");
});

test("a regression the second pass does not see is reported, and passes", () => {
  const slow = rounds([2.0, 2.1, 1.9, 2.0, 2.05], [2.6, 2.73, 2.47, 2.6, 2.67]);
  const same = rounds([2.0, 2.1, 1.9, 2.0, 2.05], [2.02, 2.08, 1.95, 2.01, 2.0]);
  const result = latencyGate({ name: BURST, passes: [slow, same] });
  assert.equal(result.verdict, "pass");
  assert.equal(result.unconfirmed, true);
});

test("one slow round is not a sustained regression, even when it is far past 20%", () => {
  // Four rounds at 22% and one at 300%: the median of the ratios is past the threshold, but a
  // regression the change made shows in every round, and only one of these is far out.
  const spiky = rounds([2.0, 2.0, 2.0, 2.0, 2.0], [2.0, 2.0, 2.44, 6.0, 2.44]);
  assert.equal(latencyGate({ name: BURST, passes: [spiky] }).verdict, "pass");
});

test("a run in which main does not agree with itself is too noisy to judge, and passes", () => {
  // main's own rounds, its slowest and fastest set aside, still spread from 2.0 to 2.5, more
  // than the threshold: the runner cannot tell a 20% change from its own noise right now, so
  // nothing it says about the change counts.
  const noisy = rounds([2.0, 2.6, 2.1, 2.0, 2.5], [2.9, 3.4, 3.0, 2.9, 3.3]);
  const result = latencyGate({ name: BURST, passes: [noisy] });
  assert.equal(result.verdict, "noisy");
  assert.equal(result.spread, 1.25);
});

test("one slow round of main's does not make the row too noisy to judge", () => {
  // A local run under load: main's first round paid for a cold cache (36 ms against 18).
  const regressed = rounds([36.0, 18.6, 18.5, 18.3, 18.0], [46.8, 24.2, 24.1, 23.8, 23.4]);
  const result = latencyGate({ name: BURST, passes: [regressed] });
  assert.equal(result.verdict, "confirm");
  assert.equal(result.spread, 1.016);
});

test("a row main cannot measure has no baseline, and passes", () => {
  // The change adds the row, or main's build of the bench failed: nothing to compare with.
  assert.equal(latencyGate({ name: BURST, passes: [] }).verdict, "no baseline");
});

test("exactly 20% worse is not more than 20% worse", () => {
  const edge = rounds([2.0, 2.0, 2.0, 2.0, 2.0], [2.4, 2.4, 2.4, 2.4, 2.4]);
  assert.equal(latencyGate({ name: BURST, passes: [edge] }).verdict, "pass");
});

test("only the two bursts are gated: main's five records agreed within 5% on each (#932)", () => {
  assert.deepEqual(GATED, ["2 MB burst, asked to drawn", "13 MB burst, asked to drawn"]);
});

test("each gated row is one the session bench prints, so a renamed row cannot drop out of the gate", () => {
  const bench = readFileSync(
    new URL("../crates/session-protocol/src/bin/charter-session-bench.rs", import.meta.url),
    "utf8",
  );
  for (const name of GATED) assert.ok(bench.includes(`"${name}"`), `${name} is not a row the bench prints`);
});

test("a confirmed regression in a burst row fails", () => {
  const slow = rounds([16.0, 16.4, 16.2, 16.1, 16.3], [20.8, 21.3, 21.1, 20.9, 21.2]);
  assert.equal(latencyGate({ name: "13 MB burst, asked to drawn", passes: [slow, slow] }).verdict, "regressed");
});

test("the keystroke row past 20% is reported as evidence, and never fails or asks for a second pass", () => {
  const slow = rounds([0.4, 0.42, 0.38, 0.4, 0.41], [0.52, 0.55, 0.5, 0.52, 0.53]);
  const once = latencyGate({ name: KEYSTROKE, passes: [slow] });
  assert.equal(once.verdict, "past 20%, evidence only");
  assert.equal(once.ratio, 1.3);
  assert.equal(latencyGate({ name: KEYSTROKE, passes: [slow, slow] }).verdict, "past 20%, evidence only");
});

test("a row the gate does not name is evidence, whatever it says", () => {
  // A new row is evidence until main's own records show it can be judged (V62).
  const same = rounds([2.0, 2.1, 1.9, 2.0, 2.05], [2.02, 2.08, 1.95, 2.01, 2.0]);
  assert.equal(latencyGate({ name: "a row added later", passes: [same] }).verdict, "pass, evidence only");
  assert.equal(latencyGate({ name: KEYSTROKE, passes: [] }).verdict, "no baseline, evidence only");
});
