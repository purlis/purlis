// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **A workflow's jobs, line by line**, for the tools' tests that read `.github/workflows/`
// (#8009): one copy, where there were five. Line by line, as the other tools read workflows: the
// tools' tests have no packages to load. `ci-push-jobs.mjs`'s `jobsOf` reads each job's keys;
// this hands back its lines, for a test to look for what it holds.

/**
 * The jobs of a workflow's text, by name (the two-space keys under `jobs:`), each its lines.
 * A comment line is dropped unless `comments` is true.
 */
export function jobLines(text, { comments = false } = {}) {
  const lines = text.split("\n");
  const out = new Map();
  let current = null;
  for (const line of lines.slice(lines.indexOf("jobs:") + 1)) {
    if (/^\S/.test(line)) break;
    if (/^ {2}[\w-]+:\s*$/.test(line)) out.set((current = line.trim().slice(0, -1)), []);
    else if (current && (comments || !line.trim().startsWith("#"))) out.get(current).push(line);
  }
  return out;
}
