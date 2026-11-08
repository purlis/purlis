// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// What the publishing workflows' provenance says it was built from, and how they are started.
// Run with `node --test tools/`.
//
// A run started by another run's end (`workflow_run`) has main's head at that moment as its
// `github.sha`, and its OIDC token, and so the SLSA provenance it signs, names that commit,
// though the run built an earlier one. So nothing that publishes is started that way: the
// release and its runner image start from the push itself, and the operator image is called by
// `operator.yml`'s run of that push, after its checks. Each verification of a provenance is
// then pinned to the commit the run built (`plan`'s `sha`), so a run whose two differ fails.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const read = (name) => readFileSync(new URL(`../.github/workflows/${name}`, import.meta.url), "utf8");
const PUBLISHING = ["release.yml", "runner-image.yml", "operator-image.yml"];

/** The jobs of a workflow's text, by name, each its lines with comments dropped. */
function jobs(text) {
  const lines = text.split("\n");
  const out = new Map();
  let current = null;
  for (const line of lines.slice(lines.indexOf("jobs:") + 1)) {
    if (/^\S/.test(line)) break;
    if (/^ {2}[\w-]+:$/.test(line)) out.set((current = line.trim().slice(0, -1)), []);
    else if (current && !line.trim().startsWith("#")) out.get(current).push(line);
  }
  return out;
}

/** The top-level `on:` keys. */
function triggers(text) {
  const lines = text.split("\n");
  const keys = [];
  for (const line of lines.slice(lines.indexOf("on:") + 1)) {
    if (line.trim() === "" || line.trim().startsWith("#")) continue;
    if (!line.startsWith(" ")) break;
    if (/^ {2}\S/.test(line)) keys.push(line.trim().replace(/:$/, ""));
  }
  return keys;
}

test("no publishing workflow is started by another run's end", () => {
  for (const name of PUBLISHING) assert.ok(!read(name).includes("workflow_run"), name);
  assert.deepEqual(triggers(read("operator-image.yml")), ["pull_request", "workflow_call"]);
  assert.deepEqual(triggers(read("runner-image.yml")), ["pull_request", "workflow_call", "workflow_dispatch"]);
});

test("every provenance check is pinned to the commit the run built", () => {
  let seen = 0;
  for (const name of PUBLISHING) {
    for (const [job, body] of jobs(read(name))) {
      const pins = body.filter((line) => line.includes("--source-digest"));
      if (pins.length === 0) continue;
      for (const pin of pins) {
        assert.match(pin.trim(), /^--source-digest "\$SHA"( \\)?$/, `${name} ${job}: ${pin.trim()}`);
        seen += 1;
      }
      // `$SHA` is `plan`'s, in the job's own env, where no step can set it to something else.
      assert.ok(body.includes("      SHA: ${{ needs.plan.outputs.sha }}"), `${name} ${job}: SHA is not plan's sha`);
      assert.ok(!body.some((line) => /^ {10}SHA:/.test(line)), `${name} ${job}: a step sets its own SHA`);
    }
  }
  // release.yml's provenance and publish, and each image's attest.
  assert.equal(seen, 4);
});

test("the operator image is published by operator.yml's run of a push to main, after its checks and kind tests", () => {
  const operator = jobs(read("operator.yml"));
  const publish = operator.get("image");
  assert.ok(publish, "operator.yml has no image job");
  assert.ok(publish.includes("    uses: ./.github/workflows/operator-image.yml"));
  assert.ok(publish.includes("    needs: [checks, kind]"));
  assert.ok(publish.includes("    if: github.event_name == 'push' && github.ref == 'refs/heads/main'"));
  // The most the called workflow's jobs ask for; each of them holds only its own.
  const at = publish.indexOf("    permissions:");
  const granted = publish.slice(at + 1).filter((line, i, all) => all.slice(0, i + 1).every((l) => /^ {6}\S/.test(l)));
  assert.deepEqual(granted.map((line) => line.trim()).sort(), [
    "attestations: write",
    "contents: read",
    "id-token: write",
    "packages: write",
  ]);
  assert.ok(!publish.some((line) => line.trim().startsWith("secrets:")), "the call passes secrets");
});

test("the operator image publishes only when called, and builds the run's own commit", () => {
  const plan = jobs(read("operator-image.yml")).get("plan");
  const script = plan.map((line) => line.trim());
  assert.ok(script.includes('called) publish=true; sha=$GITHUB_SHA ;;'), "a call does not publish its own commit");
  assert.ok(script.includes('pull_request) publish=false; sha=$PR_SHA ;;'));
  assert.ok(script.includes("EVENT: ${{ github.event_name == 'push' && github.ref == 'refs/heads/main' && 'called' || github.event_name }}"));
});
