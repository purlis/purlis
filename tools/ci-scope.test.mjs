// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The scopes CI's heavy jobs run by (#3505): sample paths to scopes, and the rules held to the
// repository as it is. Run with `node --test tools/`.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { READ_BY_CODE, SCOPES, ciToolsOf, namedIn, scopesOf, scopesOfPath } from "./ci-scope.mjs";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const CI_TOOLS = new Set(["coldstart-linux.sh", "ci-scope.mjs", "no-orphans.sh"]);
const of = (paths, event = "pull_request") =>
  [...scopesOf({ event, paths, ciToolNames: CI_TOOLS }).scopes].sort();
const all = [...SCOPES].sort();
const docsOnly = ["docs", "tools"];

test("a docs-, skills- and tools-only change runs the docs and tools' tests, nothing heavy", () => {
  // #3503's paths.
  assert.deepEqual(
    of([
      "AGENTS.md",
      "docs/agents/skills/landing-a-pr/SKILL.md",
      "tools/land.mjs",
      "tools/land.test.mjs",
    ]),
    docsOnly,
  );
  assert.deepEqual(
    of(["docs/adr/0001-x.md", "site/astro.config.mjs", "changes/a-fix.md"]),
    docsOnly,
  );
  assert.deepEqual(of(["README.md", "CONTRIBUTING.md"]), docsOnly);
});

test("a change to tools/ always runs the tools' own tests", () => {
  assert.deepEqual(of(["tools/land.mjs"]), ["tools"]);
  assert.deepEqual(of(["tools/coldstart-gate.test.mjs"]), ["tools"]);
  assert.ok(of(["tools/coldstart-linux.sh"]).includes("tools"));
});

test("a tool CI runs, and a folder of tools' data, run everything", () => {
  assert.deepEqual(of(["tools/coldstart-linux.sh"]), all);
  assert.deepEqual(of(["tools/no-orphans.sh"]), all);
  assert.deepEqual(of(["tools/ucd/16.0.0/UnicodeData.txt"]), all);
  assert.deepEqual(of(["tools/ci-scope.mjs"]), all);
});

test("CI itself, a lockfile, the toolchain and a shared config run everything", () => {
  for (const path of [
    ".github/workflows/ci.yml",
    ".github/actions/change-base/action.yml",
    ".github/PULL_REQUEST_TEMPLATE.md",
    "Cargo.lock",
    "Cargo.toml",
    "rust-toolchain.toml",
    ".nvmrc",
    "deny.toml",
    "app/package.json",
    "app/package-lock.json",
    "app/tsconfig.json",
    "app/tsconfig.node.json",
    "app/vite.config.ts",
    "app/eslint.config.js",
    "app/.prettierrc.json",
    "app/.storybook/matrix.ts",
  ]) {
    assert.deepEqual(of([path]), all, path);
  }
});

test("code runs everything", () => {
  for (const path of [
    "crates/purlis-core/src/lib.rs",
    "app/src/App.tsx",
    "app/src/App.test.tsx",
    "app/src-tauri/src/lib.rs",
    "app/src-tauri/plugin/skills/theme/SKILL.md",
    "crates/purlis-core/docs/themes.md",
    "tests/fixtures/recorded/behaviour.jsonl",
    "fixtures/corpora/a.json",
    "runner-image/Dockerfile",
    "operator/src/main.rs",
  ]) {
    assert.deepEqual(of([path]), all, path);
  }
});

test("an unknown path runs everything", () => {
  assert.deepEqual(of(["something-new/file.txt"]), all);
  assert.deepEqual(of(["LICENSE"]), all);
  assert.deepEqual(of(["docs.txt"]), all);
});

test("a doc that code reads runs everything", () => {
  for (const path of [
    "CHANGELOG.md",
    "docs/plane-format.md",
    "docs/schema/theme.v2.schema.json",
    "docs/ui-primitives.md",
  ]) {
    assert.deepEqual(of([path]), all, path);
  }
});

test("the scenario specs alone run the web checks and the scenarios, not Rust or the app", () => {
  assert.deepEqual(of(["app/e2e/specs/tabs.e2e.ts"]), ["scenario", "tools", "web"]);
});

test("scopes add up over a change's paths", () => {
  assert.deepEqual(of(["docs/a.md", "app/e2e/specs/x.e2e.ts"]), [
    "docs",
    "scenario",
    "tools",
    "web",
  ]);
  assert.deepEqual(of(["docs/a.md", "crates/host/src/lib.rs"]), all);
});

test("only a pull request is scoped: a merge group, a push and a nightly run everything", () => {
  for (const event of ["merge_group", "push", "schedule", "workflow_dispatch"]) {
    assert.deepEqual(of(["docs/a.md"], event), all, event);
  }
  assert.deepEqual(of([]), all, "a change of no paths");
});

test("every path that builds the app builds the scenarios too", () => {
  // `app builds (macos-latest)` is built in the macOS scenario job, so `app` needs `scenario`.
  for (const path of ["app/e2e/x.ts", "docs/a.md", "tools/x.mjs", "app/src/a.ts", "Cargo.lock"]) {
    const scopes = scopesOfPath(path, CI_TOOLS);
    assert.ok(!scopes.has("app") || scopes.has("scenario"), path);
    assert.ok(scopes.has("tools"), `${path}: the tools' tests run on every change`);
  }
});

test("a tool is CI's when a file outside tools/ and the docs names it, or such a tool", () => {
  const files = {
    ".github/workflows/ci.yml": "run: tools/no-orphans.sh -- cargo test",
    "app/vite.config.ts": 'import { x } from "../tools/notices.mjs";',
    "AGENTS.md": "Land with `node tools/land.mjs`.",
    "docs/landing.md": "tools/land.mjs",
    "tools/no-orphans.sh": 'node "$(dirname "$0")/orphan-report.mjs"',
    "tools/notices.mjs": "export const x = 1;",
    "tools/orphan-report.mjs": "",
    "tools/land.mjs": "// see no-orphans.sh",
    "tools/stress-no-orphans.sh": "",
  };
  const ci = ciToolsOf(Object.keys(files), (p) => files[p]);
  assert.deepEqual([...ci].sort(), ["no-orphans.sh", "notices.mjs", "orphan-report.mjs"]);
});

test("a tool's name is matched as text, whole, whatever characters it holds", () => {
  assert.ok(namedIn('node "$(dirname "$0")/bench.mjs"', "bench.mjs"));
  assert.ok(!namedIn("tools/stress-bench.mjs", "bench.mjs"));
  assert.ok(!namedIn("tools/bench.mjs2", "bench.mjs"));
  assert.ok(!namedIn("benchXmjs", "bench.mjs"), "a dot is a dot, not any character");
  for (const name of ["a+b.mjs", "c(1).sh", "d\\e.py", "f[g].mjs", "h$.mjs", "i*.mjs"]) {
    assert.ok(namedIn(`run tools/${name} now`, name), name);
    assert.ok(!namedIn(`run tools/x${name} now`, name), `x${name}`);
  }
  assert.ok(!namedIn("a+bbbb.mjs", "a+b.mjs"));
  assert.ok(!namedIn("c1.sh", "c(1).sh"));
});

// The rules held to this repository, so a new reader of a doc or a new tool CI runs fails here
// before a scoped run can skip what it needs.
const git = (...args) =>
  execFileSync("git", args, { cwd: ROOT, encoding: "utf8", maxBuffer: 1 << 28 })
    .split("\n")
    .filter(Boolean);
const read = (p) => {
  try {
    return readFileSync(new URL(`../${p}`, import.meta.url), "utf8");
  } catch {
    return "";
  }
};

test("in this repository, the tools CI runs are found, and a hand-run tool is not one", () => {
  const ci = ciToolsOf(git("ls-files"), read);
  const named = ["no-orphans.sh", "coldstart-linux.sh", "coldstart-gate.mjs", "ci-scope.mjs"];
  for (const name of named) {
    assert.ok(ci.has(name), name);
  }
  for (const name of ["tracker-rename.mjs", "adr-amendments.test.mjs"]) {
    assert.ok(!ci.has(name), name);
  }
});

// A doc the code reads by a relative path: `include_str!("../../docs/x.md")`,
// `join("..", "docs", "x.md")`, or a root Markdown file the same ways. A comment's link to a doc
// reads nothing, so comment lines are left out.
const FIXTURES_NOT_READS = new Set([
  // Paths a test refuses as outside a piece's folder; nothing reads the repository's README.
  "README.md",
]);

test("every doc the code reads by a relative path runs everything", () => {
  const lines = git(
    "grep",
    "-h",
    "-E",
    String.raw`(\.\./)+(docs/|[A-Z_]+\.md)|"\.\.", *"docs"`,
    "--",
    "crates",
    "app",
    "tests",
    "fixtures",
    ":!*.md",
  );
  // Only a path that is a doc of this repository: `"../SKILL.md"` beside a test's fixture is not.
  const tracked = new Set(git("ls-files", "--", "docs", "*.md"));
  const read = new Set();
  for (const line of lines) {
    if (/^\s*(\/\/|\*|\/\*|#)/.test(line)) continue;
    for (const [, path] of line.matchAll(/(?:\.\.\/)+((?:docs\/)?[A-Za-z0-9_./-]+\.[a-z]+)/g)) {
      if (path.startsWith("docs/") || /^[A-Z_]+\.md$/.test(path)) read.add(path);
    }
    for (const [, rest] of line.matchAll(/"\.\.", *"docs", *((?:"[^"]+", *)*"[^"]+")/g)) {
      read.add(`docs/${[...rest.matchAll(/"([^"]+)"/g)].map((m) => m[1]).join("/")}`);
    }
  }
  assert.ok(read.has("CHANGELOG.md"), "the search finds include_str!(\"../../../CHANGELOG.md\")");
  assert.ok(read.has("docs/design-system.md"), "and join(.., \"docs\", ..)");
  for (const path of read) {
    if (FIXTURES_NOT_READS.has(path) || !tracked.has(path)) continue;
    assert.deepEqual(
      [...scopesOfPath(path, new Set())].sort(),
      all,
      `${path} is read by code: add it to READ_BY_CODE in tools/ci-scope.mjs`,
    );
  }
  assert.ok(READ_BY_CODE.length > 0);
});
