// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The cluster pool guide can't drift from what CI runs (#4537). Run with `node --test tools/`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { blocks, commands } from "./guide-commands.mjs";

const repo = fileURLToPath(new URL("..", import.meta.url));
const read = (path) => readFileSync(join(repo, path), "utf8");
const guide = read("docs/cluster-pool-setup.md");
const kind = read("operator/e2e/kind.sh");

test("every command block of the guide is run on kind or says it is not", () => {
  const all = blocks(guide);
  assert.ok(all.length > 0);
  for (const block of all) {
    assert.ok(
      (block.name !== undefined) !== block.notRun,
      `the block on line ${block.line} needs a kind name or a "Not yet run in CI" line, not both`,
    );
  }
});

test("kind.sh runs every block the guide says it runs, and no other", () => {
  const named = blocks(guide).flatMap((block) => block.name ?? []);
  assert.equal(new Set(named).size, named.length, "a name is used twice");
  const ran = [...kind.matchAll(/\bguide_(?:run|text) ([a-z-]+)/g)].map((match) => match[1]);
  assert.deepEqual([...new Set(ran)].sort(), [...named].sort());
});

test("what the guide says the doctor prints is what kind.sh holds it to", () => {
  const held = kind.match(/grep -q "\^(missing: upstream agent-sandbox v1\.0\.6: not installed)"/);
  assert.ok(held, "kind.sh no longer checks the doctor's words");
  assert.ok(guide.includes(held[1]));
});

test("a block inside a comment is refused, since the page would not show what CI runs", () => {
  assert.throws(() => blocks("<!--\n```bash\nrun\n```\n-->\n"), /inside a comment/);
});

test("a command's placeholders are all put in, or it is refused", () => {
  const markdown = "<!-- kind: one -->\n\n```bash\nrun <your registry>/x <<'EOF'\nEOF\n```\n";
  assert.throws(() => commands(markdown, "one"), /still holds <your registry>/);
  assert.equal(
    commands(markdown, "one", [["<your registry>", "r"]]),
    "run r/x <<'EOF'\nEOF",
  );
  assert.throws(() => commands(markdown, "one", [["<other>", "r"]]), /has no <other>/);
  assert.throws(() => commands(markdown, "two"), /no block called two/);
});
