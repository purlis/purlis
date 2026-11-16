// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The policy docs' named checks (any-model T1a-64). Run with `node --test tools/`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { policy_docs_carry_the_rollout_advice } from "./policy-docs.mjs";

const PLANE_FORMAT = readFileSync(
  fileURLToPath(new URL("../docs/plane-format.md", import.meta.url)),
  "utf8",
);

test("policy_docs_carry_the_rollout_advice", () => {
  assert.deepEqual(policy_docs_carry_the_rollout_advice(PLANE_FORMAT), []);
});

test("the rollout advice is looked for in the policy section alone", () => {
  const section = "## `/etc/purlis/policy.json` — an administrator's policy";
  const advice =
    "No policy file should carry `min-version` or `models` until every install that reads\n" +
    "it is 1c or later. There is no extra stable release for it.\n";
  assert.deepEqual(
    policy_docs_carry_the_rollout_advice(`# Doc\n\n${section}\n\n${advice}\n## Next\n`),
    [],
  );
  // The same sentences under another heading are not the policy docs'.
  assert.equal(
    policy_docs_carry_the_rollout_advice(`# Doc\n\n${section}\n\nText.\n\n## Next\n\n${advice}`)
      .length,
    2,
  );
});
