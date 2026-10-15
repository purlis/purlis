// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// An ADR's amendment is a file of its own (#1982). Run with `node --test tools/`.
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { problems } from "./adr-amendments.mjs";

function adrs(files) {
  const root = mkdtempSync(join(tmpdir(), "adr-amendments-"));
  for (const [path, text] of Object.entries(files)) {
    mkdirSync(join(root, path, ".."), { recursive: true });
    writeFileSync(join(root, path), text);
  }
  return root;
}

const POINTER = "## Amendments\n\nThey are in [`0067/`](0067/).\n";
const ADR = `# A chat runs in a sandbox\n\n## The decision\n\nText.\n\n${POINTER}`;

test("an ADR whose amendments are files of their own passes", () => {
  const dir = adrs({
    "0067-a-sandbox.md": ADR,
    "0067/2026-10-06-a-block-is-never-a-dead-end.md": "# Amended (2026-10-06): a block\n\nText.\n",
    "0068-no-amendments.md": "# Another\n\n## The decision\n\nText.\n",
  });
  assert.deepEqual(problems(dir), []);
});

test("a dated amendment section added to an ADR is refused, naming the file to write", () => {
  const dir = adrs({
    "0067-a-sandbox.md": `${ADR}\n## Amended (2026-10-16, #2200): something new\n\nText.\n`,
    "0067/2026-10-06-a-block.md": "# Amended (2026-10-06): a block\n",
  });
  const found = problems(dir);
  assert.equal(found.length, 1);
  assert.match(found[0], /^0067-a-sandbox\.md:11: /);
  assert.match(found[0], /docs\/adr\/0067\/2026-10-16-<slug>\.md/);
});

test("each of the words an amendment's heading starts with is refused", () => {
  for (const heading of [
    "Amended 2026-10-14: a rule",
    "Amendment, 2026-09-22: a rule",
    "Amended by FD-4 (#643), 2026-10-01",
    "Noted (2026-10-13, qw346): a note",
    "Note, 2026-10-07: a note",
    "Ruled (V21, 2026-09-30)",
    "Re-opened, 2026-09-30: a sandbox is proposed",
    "Decided (dispatcher, 2026-10-01)",
    "Built: a thing (#752, 2026-10-12)",
    "ADR 0086, amended (2026-10-02)",
  ]) {
    const dir = adrs({ "0086-x.md": `# X\n\n## The decision\n\n## ${heading}\n\nText.\n` });
    assert.equal(problems(dir).length, 1, heading);
  }
});

test("an ADR with no folder yet is told to start one", () => {
  const dir = adrs({ "0090-x.md": "# X\n\n## Amended (2026-10-14): a task reports\n" });
  assert.match(problems(dir)[0], /docs\/adr\/0090\/2026-10-14-<slug>\.md/);
});

test("a heading that is not an amendment, or a dated line in a code block, passes", () => {
  const dir = adrs({
    "0064-x.md": [
      "# X",
      "## Getting a session back (SI-8d, added 2026-09-28)",
      "## Amendments",
      "## Where purlis is today",
      "```markdown",
      "## Amended 2026-10-14: an example in a fence",
      "```",
      "### Amended 2026-10-07: a sub-section is not this rule's",
      "",
    ].join("\n"),
  });
  assert.deepEqual(problems(dir), []);
});

test("an amendments folder with no ADR of its number is refused", () => {
  const dir = adrs({ "0099/2026-10-06-orphan.md": "# Amended (2026-10-06): orphan\n" });
  assert.match(problems(dir)[0], /^0099\/: /);
});

test("an ADR that has a folder and does not name it is refused", () => {
  const dir = adrs({
    "0067-a-sandbox.md": "# A sandbox\n\n## The decision\n",
    "0067/2026-10-06-a-block.md": "# Amended (2026-10-06): a block\n",
  });
  assert.match(problems(dir)[0], /^0067-a-sandbox\.md: .*\(0067\/\)/);
});

test("an amendment file is named by its date and a slug, and starts with its heading", () => {
  const dir = adrs({
    "0067-a-sandbox.md": ADR,
    "0067/a-block.md": "# Amended (2026-10-06): a block\n",
    "0067/2026-10-06-Block_Two.md": "# Amended (2026-10-06): a block\n",
    "0067/2026-10-07-no-heading.md": "Text.\n",
  });
  const found = problems(dir).join("\n");
  assert.match(found, /0067\/a-block\.md: /);
  assert.match(found, /0067\/2026-10-06-Block_Two\.md: /);
  assert.match(found, /0067\/2026-10-07-no-heading\.md: .*heading/);
});

test("the repository's own ADRs hold to the rule", () => {
  const dir = fileURLToPath(new URL("../docs/adr", import.meta.url));
  assert.deepEqual(problems(dir), []);
});
