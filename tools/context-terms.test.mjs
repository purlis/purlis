// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// `CONTEXT.md` carries the words phase 1a makes true, and no more (T1a-19, #6767). Run with
// `node --test tools/`.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { ASK_SENTENCE, TERMS, entry, missing, untrue } from "./context-terms.mjs";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const CONTEXT = readFileSync(new URL("../CONTEXT.md", import.meta.url), "utf8");

test("an entry is read from its head to the next entry, across its paragraphs", () => {
  const text = "## A\n\n**One** (of a thing):\nFirst.\n\nSecond.\n_Avoid_: x\n\n**Two**:\nThird.\n";
  assert.equal(entry(text, "One"), "**One** (of a thing):\nFirst.\n\nSecond.\n_Avoid_: x");
  assert.equal(entry(text, "Three"), null);
  // An issue number wrapped to the start of a line is not a heading.
  const wrapped = "**One**:\nAmended for\n#6578).\n_Avoid_: x\n\n### Next\n";
  assert.equal(entry(wrapped, "One"), "**One**:\nAmended for\n#6578).\n_Avoid_: x");
});

test("an entry that reads 1a's words wrapped differently passes, and one word off fails", () => {
  const [head, ...rest] = TERMS.Route.split("\n");
  const wrapped = `${head}\n${rest.join(" ").replace("at its start: a", "at its start:\na")}`;
  assert.deepEqual(
    missing(wrapped).filter((one) => one.includes("Route")),
    [],
  );
  const off = TERMS.Route.replace("another route is another run", "another route is the same run");
  assert.match(missing(off).join("\n"), /\*\*Route\*\* entry isn't 1a's/);
});

test("a later phase's clause and a bare proxy are each named", () => {
  const later = `${TERMS["Your keys"].replace("system keychain.", "system keychain, or a reference to an entry in 1Password.")}\n\n**Ask**:\nA thing.\n`;
  assert.match(untrue(later).join("\n"), /\*\*Your keys\*\* says "1Password"/);
  const bare = TERMS["Internet access"].replace("the **key\nproxy**", "the proxy");
  assert.match(untrue(bare).join("\n"), /\*\*Internet access\*\* says "the proxy"/);
});

test("context_md_carries_the_1a_terms", () => {
  // R-008, R-009, R-011, R-014, R-018: Route, Key proxy, Your keys, and the replaced Harness
  // declaration and Internet access, word for word within 1a (design §2.2, §2.3). Block is
  // T1a-26's.
  assert.ok(!Object.keys(TERMS).includes("Block"));
  assert.deepEqual(missing(CONTEXT), []);
});

test("context_md_lands_only_1a_true_parts", () => {
  // U22: no later-phase clause; Sign-in kind and Model source no longer say a vault; Ask is
  // keyed by what is missing; pool key allowance is neither a grant nor an approval.
  assert.deepEqual(untrue(CONTEXT), []);
  assert.ok(entry(CONTEXT, "Ask").includes(ASK_SENTENCE.split(" ").slice(0, 6).join(" ")));
  assert.ok(entry(CONTEXT, "Sign-in kind").includes("**your keys**"));
  assert.ok(entry(CONTEXT, "Model source").includes("**your keys**"));
});

test("no_ui_string_says_ai_providers", () => {
  // R-661: the preset is *Model providers* everywhere the window or its core says it.
  // `git grep` answers 1 for no line found, 0 with the lines, and anything else for trouble.
  const run = spawnSync(
    "git",
    ["grep", "-n", "-F", "AI providers", "--", "crates", "app/src", "app/src-tauri"],
    { cwd: ROOT, encoding: "utf8" },
  );
  assert.equal(run.stdout, "");
  assert.equal(run.status, 1, run.stderr);
});
