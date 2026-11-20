// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Refreshes the snapshot of GitHub's published web, api and git ranges that a chat's egress proxy
// holds beside SNI pinning (ADR 0128 D3g, #7292): `crates/purlis-core/src/sandbox/forgedeny/
// github-ranges.txt`, read by `include_str!`, never fetched at runtime from a chat's path.
//
// Run `node tools/github-ranges-refresh.mjs` by hand and commit the diff. It reads
// `https://api.github.com/meta` (or a saved copy: `node tools/github-ranges-refresh.mjs meta.json`).
// Pages, Actions, Codespaces, Copilot, Packages and the rest carry other people's sites or
// addresses shared with them, and the hooks list is where GitHub's webhooks come *from*, so
// none of them is held.
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

/** The lists of `/meta` that are where a chat could sign in to GitHub. */
export const LISTS = ["web", "api", "git"];

export const SNAPSHOT = fileURLToPath(
  new URL(
    "../crates/purlis-core/src/sandbox/forgedeny/github-ranges.txt",
    import.meta.url,
  ),
);

/** The snapshot's text for one answer of `/meta`: a CIDR a line, sorted, without repeats. */
export function render(meta) {
  const ranges = new Set();
  for (const list of LISTS) {
    if (!Array.isArray(meta[list]) || meta[list].length === 0) {
      throw new Error(`/meta has no "${list}" list`);
    }
    for (const range of meta[list]) {
      if (!/^[0-9a-f.:]+\/\d{1,3}$/i.test(range)) {
        throw new Error(`"${range}" in "${list}" is not a CIDR`);
      }
      ranges.add(range.toLowerCase());
    }
  }
  const sorted = [...ranges].sort();
  return (
    `# GitHub's published ${LISTS.join(", ")} ranges (api.github.com/meta).\n` +
    `# Refreshed by tools/github-ranges-refresh.mjs; never edit by hand.\n` +
    sorted.map((range) => `${range}\n`).join("")
  );
}

async function main() {
  const [saved] = process.argv.slice(2);
  const meta = saved
    ? JSON.parse(readFileSync(saved, "utf8"))
    : await (await fetch("https://api.github.com/meta")).json();
  writeFileSync(SNAPSHOT, render(meta));
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  await main();
}
