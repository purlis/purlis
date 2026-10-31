// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { CAP, FLOOR, STEP, TRIGGER, measured, outputs, partsFor } from "./story-shards.mjs";

const shard = (name, seconds, conclusion = "success", step = STEP) => ({
  name,
  steps: [
    {
      name: step,
      conclusion,
      started_at: "2026-11-01T00:00:00Z",
      completed_at: new Date(Date.parse("2026-11-01T00:00:00Z") + seconds * 1000).toISOString(),
    },
  ],
});

test("a step under the trigger keeps the parts the stories run in", () => {
  assert.equal(partsFor(4, TRIGGER - 60), 4);
  assert.equal(partsFor(4, TRIGGER), 4);
});

test("a step past the trigger adds as many parts as bring it back under", () => {
  assert.equal(partsFor(3, TRIGGER + 1), 4);
  assert.equal(partsFor(3, TRIGGER * 2), 6);
});

test("the parts settle: the run after a split does not split or join again", () => {
  const next = partsFor(3, TRIGGER * 1.1);
  assert.equal(next, 4);
  assert.equal(partsFor(next, (TRIGGER * 1.1 * 3) / next), next);
});

test("the parts stay between the floor and the cap", () => {
  assert.equal(partsFor(6, 1), FLOOR);
  assert.equal(partsFor(CAP, TRIGGER * 100), CAP);
});

test("a run's measure is its slowest stories step, in whichever browser", () => {
  assert.deepEqual(
    measured([
      shard("stories (chromium 1/3)", 200),
      shard("stories (webkit 2/3)", 465),
      shard("stories (webkit 3/3)", 376),
    ]),
    { of: 3, slowest: 465 },
  );
});

test("only a shard's own finished stories step is measured", () => {
  assert.equal(
    measured([
      shard("what this change touches", 900),
      shard("stories", 900),
      shard("stories (webkit 1/3)", 900, "failure"),
      shard("stories (webkit 2/3)", 900, "success", "npm ci"),
      { name: "stories (webkit 3/3)" },
    ]),
    undefined,
  );
});

test("the outputs are the parts the workflow's matrix reads", () => {
  assert.equal(outputs(4), "shards=[1,2,3,4]\nof=4\n");
});

test("the workflow's step is the one measured, and its matrix falls back to the floor", () => {
  const workflow = readFileSync(
    new URL("../.github/workflows/stories.yml", import.meta.url),
    "utf8",
  );
  assert.ok(workflow.includes(`- name: ${STEP}\n`));
  const floor = JSON.stringify(Array.from({ length: FLOOR }, (_, at) => at + 1));
  assert.ok(workflow.includes(`fromJson(needs.changes.outputs.shards || '${floor}')`));
  assert.ok(workflow.includes(`fromJson(needs.changes.outputs.of || '${FLOOR}')`));
});
