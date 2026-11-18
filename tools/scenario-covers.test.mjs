// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Running the fixture and e2e covers (M74.1, T1.1-54). The test name is the ticket's.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { commandOf, coversOf, packageOf, recordOf, runCovers, specHasTest } from "./scenario-covers.mjs";

const root = fileURLToPath(new URL("..", import.meta.url));

const list = {
  rows: [
    { row: "M-A-01", dialect: "claude", os: "linux", kind: "scenario", cover: "scenario: cells/B0.json#M-A-01" },
    { row: "M-WIN-02", dialect: "claude", os: "macos", kind: "e2e", cover: "e2e: app/e2e/specs/focus.e2e.ts#focus returns" },
    { row: "M-PRO-01", dialect: "claude", os: "linux", kind: "fixture", cover: "fixture: crates/purlis-core/tests/it/policy.rs::a_policy_is_read" },
    { row: "M-X-09", os: "linux", kind: "fixture", cover: "pending: 19" },
  ],
  fixtures: [
    { row: "M-WAY-04", os: "linux", reason: "race", cover: "fixture: crates/purlis-core/tests/it/race.rs::the_race" },
    { row: "M-WAY-04", os: "macos", reason: "race", cover: "fixture: crates/purlis-core/tests/it/race.rs::the_race" },
  ],
};

test("every_named_fixture_and_e2e_cover_runs_on_its_os_and_reports", () => {
  // Each cover is picked for the OS it names, and only for it; scenario and pending covers run nothing.
  const linux = coversOf(list, "linux");
  assert.deepEqual(linux.map((c) => `${c.kind} ${c.row} ${c.test}`), [
    "fixture M-PRO-01 a_policy_is_read",
    "fixture M-WAY-04 the_race",
  ]);
  const mac = coversOf(list, "macos");
  assert.deepEqual(mac.map((c) => `${c.kind} ${c.row} ${c.test}`), ["e2e M-WIN-02 focus returns", "fixture M-WAY-04 the_race"]);

  // The commands: a fixture by package and name, an e2e through the WebdriverIO setup.
  const [program, args] = commandOf(linux[0], root);
  assert.equal(program, "cargo");
  assert.ok(args.includes("--no-tests=fail") && args.includes("--retries") && args.includes("purlis-core"), args.join(" "));
  const [npm, npmArgs, cwd] = commandOf(mac[0], root);
  assert.equal(npm, "npm");
  assert.deepEqual(npmArgs.slice(0, 4), ["run", "e2e", "--", "--spec"]);
  assert.ok(cwd.endsWith("/app"));

  // Each reports in ticket 5's schema: a pass, a fail, and a test that ran nothing is not run.
  const seen = [];
  const exits = [0, 1];
  const records = runCovers(list, "linux", { exec: (p, a) => (seen.push(a.join(" ")), exits.shift()) });
  assert.equal(seen.length, 2, "both ran");
  assert.deepEqual(records.map((r) => r.verdict), ["pass", "fail"]);
  for (const r of records) {
    assert.deepEqual(Object.keys(r).sort(), ["kind", "leg", "os", "row", "test", "timings", "verdict"]);
    assert.equal(r.leg, r.kind);
  }
  const none = recordOf(linux[0], 4);
  assert.equal(none.verdict, "not_run");
  assert.match(none.reason, /ignored, filtered out/);
  // An e2e that fails is a fail, whatever its code.
  assert.equal(recordOf(mac[0], 4).verdict, "fail");

  // The real list names no package it cannot find, whichever covers it holds.
  const real = JSON.parse(readFileSync(join(root, "tests/scenarios/coverage.json"), "utf8"));
  for (const os of ["linux", "macos"]) for (const c of coversOf(real, os)) if (c.kind === "fixture") packageOf(c.file, root);
  assert.equal(packageOf("crates/purlis-core/tests/it/x.rs", root), "purlis-core");
});

test("an_e2e_cover_whose_test_the_spec_does_not_name_is_not_run_and_never_a_pass", () => {
  let ran = 0;
  const exec = () => (ran += 1, 0);
  const [missing] = runCovers(list, "macos", { kind: "e2e", exec, has: () => false });
  assert.equal(missing.verdict, "not_run");
  assert.match(missing.reason, /no test ran/);
  assert.equal(ran, 0, "nothing ran for a test the spec does not name");
  const [present] = runCovers(list, "macos", { kind: "e2e", exec, has: () => true });
  assert.equal(present.verdict, "pass");
  // Every e2e cover of the real list names a test its spec has.
  const real = JSON.parse(readFileSync(join(root, "tests/scenarios/coverage.json"), "utf8"));
  for (const os of ["linux", "macos"]) for (const c of coversOf(real, os)) if (c.kind === "e2e") assert.ok(specHasTest(c, root), `${c.spec}: ${c.test}`);
});
