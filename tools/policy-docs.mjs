// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The policy docs' review-only rows, each a named check (any-model T1a-64, polish D-14): a review
// alone can't fail CI, so the sentence the row asks for is looked for where it belongs, and its
// absence is a failure. T1a-48's script runs these by name and never redefines them (D-5, D-11).
// CI runs them through `tools/policy-docs.test.mjs`; `node tools/policy-docs.mjs` runs them by
// hand against this checkout.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const PLANE_FORMAT = fileURLToPath(new URL("../docs/plane-format.md", import.meta.url));
const POLICY_HEADING = "## `/etc/purlis/policy.json`";

/** `docs/plane-format.md`'s policy section: its heading to the next `## ` heading. */
export function policySection(text) {
  const start = text.indexOf(`\n${POLICY_HEADING}`);
  if (start < 0) return null;
  const next = text.indexOf("\n## ", start + 1);
  return text.slice(start, next < 0 ? text.length : next);
}

/** A text's words on one line each, so a sentence wrapped across lines is still found. */
function flat(text) {
  return text.replace(/\s+/g, " ");
}

/**
 * **R-431: the rollout advice is published.** The policy section says no policy file should
 * carry `min-version` or `models` until every install that reads it is 1c or later, and that no
 * extra stable release is made for it. Answers the problems found, none when it holds.
 */
export function policy_docs_carry_the_rollout_advice(text = readFileSync(PLANE_FORMAT, "utf8")) {
  const section = policySection(text);
  if (section === null) return [`docs/plane-format.md has no "${POLICY_HEADING}" section`];
  const said = flat(section);
  const wanted = [
    "No policy file should carry `min-version` or `models` until every install that reads it is 1c or later.",
    "There is no extra stable release for it.",
  ];
  return wanted
    .filter((sentence) => !said.includes(sentence))
    .map((sentence) => `docs/plane-format.md's policy section doesn't say: ${sentence}`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const problems = policy_docs_carry_the_rollout_advice();
  for (const problem of problems) console.error(problem);
  process.exit(problems.length === 0 ? 0 : 1);
}
