// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The scopes CI's heavy jobs run by (#3505): sample paths to scopes, and the rules held to the
// repository as it is. Run with `node --test tools/`.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, posix } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import {
  AGENT_STEERING,
  DOCS_READ_BY_RUST,
  GATES,
  RUST_READERS,
  READ_BY_CODE,
  READ_BY_RUST,
  READ_BY_WEB,
  SCOPES,
  WHOLE_TREE_GUARDS,
  WINDOW_SOURCE_GUARDS,
  ciToolsOf,
  gatesOf,
  namedIn,
  scopesOf,
  scopesOfPath,
  webParts,
} from "./ci-scope.mjs";

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
  assert.deepEqual(of(["CLAUDE.md"]), docsOnly);
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
    "app/src-tauri/plugin/skills/theme/SKILL.md",
    "tests/fixtures/recorded/behaviour.jsonl",
    "fixtures/corpora/a.json",
    "runner-image/Dockerfile",
  ]) {
    assert.deepEqual(of([path]), all, path);
  }
});

// #3596: of the last 120 merged pull requests, 28 changed only `app/` outside `app/src-tauri/`, and
// each ran the whole Rust suite.
const window = ["app", "scenario", "stories", "tools", "web"];

test("the window's sources run the web checks, the app and the scenarios, not Rust", () => {
  for (const path of [
    "app/src/App.tsx",
    "app/src/App.test.tsx",
    "app/src/ui/button/Button.tsx",
    "app/src/settings/project.ts",
  ]) {
    assert.deepEqual(of([path]), window, path);
  }
  assert.deepEqual(of(["app/src/App.tsx", "docs/a.md"]), [...window, "docs"].sort());
});

test("a file of the window's sources that Rust reads runs everything", () => {
  for (const path of [
    "app/src/bindings.ts",
    "app/src/uiRpc.ts",
    "app/src/theme/theme.ts",
    "app/src/ui/button/button.css",
    "app/src/App.css",
  ]) {
    assert.deepEqual(of([path]), all, path);
  }
  assert.deepEqual(of(["app/src/App.tsx", "app/src/bindings.ts"]), all);
});

test("every file at the root runs everything but an agent's steering", () => {
  // `community.test.ts` reads README.md, CONTRIBUTING.md, SUPPORT.md, CODE_OF_CONDUCT.md and the
  // SECURITY.md they link to; the app's tests read LICENSE and NOTICE.
  for (const path of git("ls-files").filter((p) => !p.includes("/"))) {
    const want = AGENT_STEERING.has(path) ? docsOnly : all;
    assert.deepEqual(of([path]), want, path);
  }
  for (const path of ["README.md", "CONTRIBUTING.md", "SUPPORT.md", "CODE_OF_CONDUCT.md"]) {
    assert.deepEqual(of([path]), all, path);
  }
  assert.deepEqual(of(["SECURITY.md", "LICENSE", "NOTICE", "CONTEXT.md"]), all);
  assert.deepEqual(of(["AGENTS.md", "CLAUDE.md"]), docsOnly);
});

test("a doc deleted or renamed runs everything: a root file a test holds may link to it", () => {
  const paths = ["docs/platforms.md", "docs/supported-platforms.md"];
  const deleted = new Set(["docs/platforms.md"]);
  const scopes = scopesOf({ event: "pull_request", paths, ciToolNames: CI_TOOLS, deleted });
  assert.deepEqual([...scopes.scopes].sort(), all);
  assert.deepEqual(of(["docs/platforms.md"]), docsOnly, "an edit stays narrow");
});

// #3520: `CONTRIBUTING.md` links `changes/README.md`, and `community.test.ts` fails on a dead link,
// so a file deleted from any folder or file that runs the docs alone runs everything.
test("a file deleted from changes/, site/ or an agent's steering runs everything", () => {
  for (const path of [
    "changes/README.md",
    "changes/a-fix.md",
    "site/astro.config.mjs",
    "site/src/pages/index.astro",
    "docs/platforms.md",
    "AGENTS.md",
    "CLAUDE.md",
  ]) {
    const deleted = new Set([path]);
    const scopes = scopesOf({
      event: "pull_request",
      paths: [path],
      ciToolNames: CI_TOOLS,
      deleted,
    });
    assert.deepEqual([...scopes.scopes].sort(), all, `${path} deleted`);
    assert.deepEqual(scopes.why, [path], `${path} is named as why`);
    assert.deepEqual(of([path]), docsOnly, `${path} edited stays narrow`);
  }
  // A rename is a deletion and an addition (`--no-renames`).
  const renamed = scopesOf({
    event: "pull_request",
    paths: ["changes/README.md", "changes/index.md"],
    ciToolNames: CI_TOOLS,
    deleted: new Set(["changes/README.md"]),
  }).scopes;
  assert.deepEqual([...renamed].sort(), all);
});

// #3596: a deletion runs everything only where its edit runs the docs alone; a window or e2e
// rename keeps its own scopes, so a refactor of `app/src/` runs no Rust.
test("a file deleted from app/src/ or app/e2e/ keeps the scopes its edit runs", () => {
  const window = scopesOf({
    event: "pull_request",
    paths: ["app/src/App.tsx", "app/src/Shell.tsx"],
    ciToolNames: CI_TOOLS,
    deleted: new Set(["app/src/App.tsx"]),
  }).scopes;
  assert.deepEqual([...window].sort(), ["app", "scenario", "stories", "tools", "web"]);
  const e2e = scopesOf({
    event: "pull_request",
    paths: ["app/e2e/specs/tabs.e2e.ts"],
    ciToolNames: CI_TOOLS,
    deleted: new Set(["app/e2e/specs/tabs.e2e.ts"]),
  }).scopes;
  assert.deepEqual([...e2e].sort(), ["scenario", "tools", "web"]);
});

test("an unknown path runs everything", () => {
  assert.deepEqual(of(["something-new/file.txt"]), all);
  assert.deepEqual(of(["LICENSE"]), all);
  assert.deepEqual(of(["docs.txt"]), all);
});

test("a doc that code reads runs everything", () => {
  for (const path of [
    "CHANGELOG.md",
    "docs/ui-primitives.md",
  ]) {
    assert.deepEqual(of([path]), all, path);
  }
  for (const path of ["docs/plane-format.md", "docs/schema/theme.v2.schema.json"]) {
    assert.deepEqual(of([path]), ["app", "docs", "rust", "scenario", "tools"], path);
  }
});

test("no file of the window's checks reads a doc only Rust's tests read", () => {
  const sources = git("ls-files", "--", "app/src", "app/e2e", "app/.storybook").filter((f) =>
    /\.(ts|tsx|mts|js|mjs)$/.test(f),
  );
  assert.ok(sources.length > 0);
  for (const file of sources) {
    const text = read(file)
      .replace(/\/\*[\s\S]*?\*\//g, "")
      .replace(/^\s*\/\/.*$/gm, "");
    for (const doc of DOCS_READ_BY_RUST) {
      const name = posix.basename(doc);
      const pieces = name === "schema" ? /"docs",\s*"schema"|docs\/schema/ : new RegExp(name.replace(".", "\\."));
      assert.ok(!pieces.test(text), `${file} reads ${doc}: take it out of DOCS_READ_BY_RUST`);
    }
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
  assert.deepEqual(of(["docs/a.md", "app/src-tauri/plugin/a.md"]), all);
});

test("only a pull request is scoped: a merge group, a push and a nightly run everything", () => {
  for (const event of ["merge_group", "push", "schedule", "workflow_dispatch"]) {
    assert.deepEqual(of(["docs/a.md"], event), all, event);
  }
  assert.deepEqual(of([]), all, "a change of no paths");
});

test("every path that builds the app builds the scenarios too", () => {
  // A change the app is built from changes what the scenarios drive, so `app` needs `scenario`.
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

test("a tool deleted or renamed runs everything, though nothing tracked names it now", () => {
  // The change deletes `tools/npm-licences.mjs` while `ci.yml`, untouched, still runs it: the file
  // is gone from `git ls-files`, so the search for CI's tools cannot find it.
  const files = {
    ".github/workflows/ci.yml": "run: node ../tools/npm-licences.mjs",
    "tools/land.mjs": "",
  };
  const ciToolNames = ciToolsOf(Object.keys(files), (p) => files[p] ?? "");
  assert.ok(!ciToolNames.has("npm-licences.mjs"));
  const paths = ["tools/npm-licences.mjs"];
  const deleted = new Set(paths);
  const scopes = scopesOf({ event: "pull_request", paths, ciToolNames, deleted }).scopes;
  assert.deepEqual([...scopes].sort(), all);
  // A rename is a deletion and an addition (`--no-renames`).
  const renamed = scopesOf({
    event: "pull_request",
    paths: ["tools/land.mjs", "tools/landing.mjs"],
    ciToolNames,
    deleted: new Set(["tools/land.mjs"]),
  }).scopes;
  assert.deepEqual([...renamed].sort(), all);
  // A hand-run tool edited, not deleted, stays narrow.
  const edited = scopesOf({ event: "pull_request", paths: ["tools/land.mjs"], ciToolNames });
  assert.deepEqual([...edited.scopes], ["tools"]);
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
// `join("..", "docs", "x.md")`, or a root Markdown file the same ways. A climb into a folder of
// the docs, with or without its trailing slash (`"../../docs"`, `"../../docs/adr/"`,
// `join("..", "docs")`), is a folder the code may list and read every file of (#3520). A
// comment's link to a doc reads nothing, so comment lines are left out. Not found: a climb in a
// backtick template (`${root}/../../docs`) and a chained `.join("..").join("docs")`; neither is
// in the tree today.
const DOCS_READ_SEARCH = String.raw`(\.\./)+(docs[/"']|[A-Z_]+\.md)|"\.\.", *"docs"`;
function docsReadIn(lines) {
  const read = new Set();
  for (const line of lines) {
    if (/^\s*(\/\/|\*|\/\*|#)/.test(line)) continue;
    for (const [, path] of line.matchAll(/(?:\.\.\/)+((?:docs\/)?[A-Za-z0-9_./-]+\.[a-z]+)/g)) {
      if (path.startsWith("docs/") || /^[A-Z_]+\.md$/.test(path)) read.add(path);
    }
    // A folder: no segment after `docs` holds a dot, and the literal ends there.
    for (const [, path] of line.matchAll(/(?:\.\.\/)+(docs(?:\/[A-Za-z0-9_-]+)*)\/?["']/g)) {
      read.add(`${path}/`);
    }
    for (const [, rest] of line.matchAll(/"\.\.", *"docs"((?:, *"[^"]+")*)/g)) {
      const parts = [...rest.matchAll(/"([^"]+)"/g)].map((m) => m[1]);
      const last = parts.at(-1) ?? "";
      read.add(["docs", ...parts].join("/") + (last.includes(".") ? "" : "/"));
    }
  }
  return read;
}

test("the docs search finds a file and a folder of the docs, with or without a slash", () => {
  const found = docsReadIn([
    'include_str!("../../../CHANGELOG.md")',
    'include_str!("../../docs/plane-format.md")',
    'let d = root.join("../../docs"); for e in std::fs::read_dir(d) {}',
    "for e in std::fs::read_dir('../../docs/adr') {}",
    'let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/");',
    'join("..", "docs", "design-system.md")',
    'join("..", "docs")',
    'join("..", "docs", "guides")',
    '// include_str!("../../docs/a-comment.md")',
  ]);
  assert.deepEqual([...found].sort(), [
    "CHANGELOG.md",
    "docs/",
    "docs/adr/",
    "docs/design-system.md",
    "docs/guides/",
    "docs/plane-format.md",
    "docs/schema/",
  ]);
});

test("every doc the code reads by a relative path runs everything", () => {
  const lines = git(
    "grep",
    "-h",
    "-E",
    DOCS_READ_SEARCH,
    "--",
    "crates",
    "app",
    "tests",
    "fixtures",
    ":!*.md",
  );
  // Only a path that is a doc of this repository: `"../SKILL.md"` beside a test's fixture is not.
  const tracked = git("ls-files", "--", "docs", "*.md");
  const trackedFiles = new Set(tracked);
  const read = docsReadIn(lines);
  assert.ok(read.has("CHANGELOG.md"), 'the search finds include_str!("../../../CHANGELOG.md")');
  assert.ok(read.has("docs/design-system.md"), 'and join(.., "docs", ..)');
  for (const path of read) {
    // A folder is read whole: every file in it is a file the code reads.
    const files = path.endsWith("/")
      ? tracked.filter((f) => f.startsWith(path))
      : trackedFiles.has(path)
        ? [path]
        : [];
    for (const file of files) {
      if (DOCS_READ_BY_RUST.some((d) => (d.endsWith("/") ? file.startsWith(d) : file === d))) {
        assert.ok(scopesOfPath(file, new Set()).has("rust"), file);
        continue;
      }
      assert.deepEqual(
        [...scopesOfPath(file, new Set())].sort(),
        all,
        `${file} is read by code (${path}): add ${path} to READ_BY_CODE in tools/ci-scope.mjs`,
      );
    }
  }
  assert.ok(READ_BY_CODE.length > 0);
});

// A file of code or tests that holds the repository's root: it names a place it is anchored at
// (`CARGO_MANIFEST_DIR`, `import.meta.url`, `process.cwd()`) and climbs from it to the root
// itself, not into a folder below it: `"../.."` or `"../../"` (a `.join` on the next line, a
// `resolve(…, "../..")`, a root held in a variable), `"..", ".."` with nothing after it,
// `process.cwd(), ".."` from `app/`, or `import.meta.glob("../../*…")`. A climb into a folder of
// the docs, `"../../docs"` with or without its slash and then a read of the folder, is not one:
// the docs search above finds it, and holds every file in that folder to `READ_BY_CODE` (#3520).
const ANCHORED = /CARGO_MANIFEST_DIR|import\.meta\.url|process\.cwd\(\)/;
const CLIMBS = new RegExp(
  [
    String.raw`"\.\./\.\./?"`,
    String.raw`"\.\.", *"\.\."(?!, *")`,
    String.raw`process\.cwd\(\), *"\.\."\s*\)`,
    String.raw`glob\(\s*\[?\s*"\.\./\.\./\*`,
  ].join("|"),
);
const WALKS = /read_dir|"ls-files"|readdirSync|import\.meta\.glob/;
const SOURCE = /\.(rs|ts|tsx|js|mjs)$/;
const rootedFiles = () =>
  git("ls-files", "--", "crates", "app", "tests")
    .filter((p) => SOURCE.test(p))
    .filter((p) => {
      const text = read(p);
      return ANCHORED.test(text) && CLIMBS.test(text);
    });

// A file that holds the root and names an agent's steering file by its bare name, with the
// reason it is not a read of the repository's own (a fixture's file of that name). None today.
const STEERING_NOT_READ = {};

test("no file that holds the repository's root reads an agent's steering file by name", () => {
  const naming = [];
  for (const path of rootedFiles()) {
    const text = read(path);
    for (const name of AGENT_STEERING) {
      if (text.includes(`"${name}"`) || text.includes(`'${name}'`)) naming.push(path);
    }
  }
  for (const path of Object.keys(STEERING_NOT_READ)) {
    assert.ok(naming.includes(path), `${path} no longer names one: take it out`);
  }
  for (const path of naming) {
    assert.ok(
      path in STEERING_NOT_READ,
      `${path} holds the root and names an agent's steering file: if it reads the ` +
        "repository's own, take it out of AGENT_STEERING in tools/ci-scope.mjs",
    );
  }
});

// A test that walks the repository from its root reads files of every scope: it is either a
// guard, which `recorded` runs on every change (`WHOLE_TREE_GUARDS`, `WINDOW_SOURCE_GUARDS`), or
// it is named here as walking only code whose changes run every scope (`crates/`,
// `app/src-tauri/`; not `app/src/`, which runs no Rust since #3596).
const WALK_ONLY_CODE = {
  "app/src-tauri/src/savefix/tests.rs":
    "crates/ and app/src-tauri/ sources; its app/src walk is in WINDOW_WALK_MISSES (#3609)",
  "app/src-tauri/src/tests.rs": "reads LICENSE and NOTICE, which run everything",
  "app/src/community.test.ts": "root files and .github/, which run everything",
  "app/src/copy.rust.test.ts": "crates/purlis-core/src and app/src-tauri/src",
  "app/src/theme/themeIds.test.ts": "app/src and app/e2e (both run web)",
  "crates/purlis-cli/tests/it/every_command_charter_names_is_one_it_has.rs":
    "crates/*/src, app/src-tauri/src and the plugin's skills",
  "crates/purlis-core/src/doctor/shadowed/tests.rs": "a plane's docs/; purlis's own is exempt",
  "crates/purlis-core/tests/it/every_git_call_goes_through_the_runner.rs":
    "crates/ and app/src-tauri/",
  "crates/purlis-core/tests/it/four_unsafe_blocks.rs": "crates/ and app/src-tauri/ Rust",
  "crates/purlis-core/tests/it/only_the_resolver_builds_the_native_transport.rs": "shipped Rust",
  "crates/purlis-core/tests/it/only_the_updater_and_report_name_charters_own_address.rs":
    "shipped Rust",
  "crates/purlis-core/tests/shipped_literals/mod.rs": "crates/ and app/src-tauri/ Rust",
  "crates/purlis-core/tests/it/the_local_layer_has_one_reader.rs": "crates/ and app/src-tauri/ Rust",
};

test("a test that walks the repository from its root is a whole-tree guard or walks code", () => {
  const walkers = rootedFiles().filter((p) => WALKS.test(read(p)));
  const guards = GUARD_FILES;
  for (const guard of WHOLE_TREE_GUARDS) {
    const path = guardFile(guard);
    assert.ok(walkers.includes(path), `${path} is found as a walker`);
  }
  for (const path of Object.keys(WALK_ONLY_CODE)) {
    assert.ok(walkers.includes(path), `${path} no longer walks from the root: take it out`);
  }
  for (const path of walkers) {
    assert.ok(
      guards.has(path) || path in WALK_ONLY_CODE,
      `${path} walks the repository from its root: add it to WHOLE_TREE_GUARDS in ` +
        "tools/ci-scope.mjs if it reads docs, tools or other files outside code, else to " +
        "WALK_ONLY_CODE here",
    );
  }
});

test("the recorded job runs every guard when Rust is out of scope", () => {
  const ci = read(".github/workflows/ci.yml");
  const step = ci.slice(ci.indexOf("- name: The whole-tree guards"));
  assert.ok(step.length < ci.length, "ci.yml has the step");
  const run = step.slice(0, step.indexOf("\n      - "));
  assert.match(run, /needs\.changes\.outputs\.rust == 'false'/);
  for (const { crate, test: name } of [...WHOLE_TREE_GUARDS, ...WINDOW_SOURCE_GUARDS]) {
    assert.ok(run.includes(`-p ${crate} --test it --`), crate);
    assert.ok(new RegExp(`\\s${name}::(\\s|$)`).test(run), name);
  }
});

// A guard is a module of its crate's one test binary (#3613): a name that is no file there
// would be a filter that matches nothing, and `cargo test` passes a filter that matches nothing.
test("every guard the recorded job runs is a test file of its crate's one binary", () => {
  for (const guard of [...WHOLE_TREE_GUARDS, ...WINDOW_SOURCE_GUARDS]) {
    assert.ok(existsSync(posix.join(ROOT, guardFile(guard))), `${guardFile(guard)} is not there`);
  }
});

// Every path under `app/src/` that Rust names (#3596): a string in a Rust file of the workspace
// that, read from the file's folder (`include_str!`), its crate's folder (`CARGO_MANIFEST_DIR`)
// or the repository's root (`root.join(..)`), is a tracked file or folder of the window's
// sources. A file named is read by Rust: it is in `READ_BY_RUST`, or the test naming it is a guard
// `recorded` runs, or it is named below with why it is not read. A folder named is walked: the
// test is a guard, or it is a known miss below. Comment lines read nothing.
const guardFile = ({ crate, test: name }) => `crates/${crate}/tests/it/${name}.rs`;
const GUARD_FILES = new Set([...WHOLE_TREE_GUARDS, ...WINDOW_SOURCE_GUARDS].map(guardFile));

// A Rust file that names a file of the window's sources without reading it, and why.
const NAMED_NOT_READ = {
  "crates/purlis-core/src/files/find.rs": "a fuzzy match's sample path, matched as text",
  "crates/purlis-core/tests/it/a_file_is_found_by_fuzzy_name.rs":
    "sample paths written into a fixture repository",
};

// A Rust test that walks the window's sources but is not run when Rust is out of scope, and why:
// a change that breaks it is caught by the merge group, which runs every scope.
const WINDOW_WALK_MISSES = {
  "app/src-tauri/src/savefix/tests.rs":
    "a unit test of the app crate, whose test build needs Tauri: one call site of mergeSaveRequest",
};

// `files` is every tracked path, `read` reads one. Each string literal of a Rust file is read
// from the file's folder (`include_str!`), its crate's folder (`CARGO_MANIFEST_DIR`, and
// `"../src"` from it), the root, and the folder above its crate when the file climbs there with
// `.parent()` from `CARGO_MANIFEST_DIR`. A path built from pieces (`root.join("app").join("src")`,
// `["app", "src"]`) is no one literal, so a file with `"app"` and `"src"` as literals of their
// own within three lines of each other, that lists or reads files, is taken as walking `app/src`.
const SPLIT_READS = /read_dir|"ls-files"|read_to_string|fs::read\b|WalkDir/;
function windowNamesIn(files, read) {
  const tracked = new Set(files);
  const folders = new Set();
  for (const f of files) {
    for (let d = posix.dirname(f); d !== "."; d = posix.dirname(d)) folders.add(d);
  }
  const crateOf = (f) => {
    let d = posix.dirname(f);
    while (d !== "." && !tracked.has(`${d}/Cargo.toml`)) d = posix.dirname(d);
    return d;
  };
  const named = [];
  for (const file of files.filter((p) => p.endsWith(".rs"))) {
    const code = read(file)
      .split("\n")
      .filter((line) => !/^\s*\/\//.test(line));
    const crate = crateOf(file);
    const whole = code.join("\n");
    // `.parent()` on `CARGO_MANIFEST_DIR` itself, on its line or the two after it.
    const climbs = code.some(
      (line, i) =>
        line.includes("CARGO_MANIFEST_DIR") &&
        code.slice(i, i + 3).some((next) => next.includes(".parent()")),
    );
    const places = [posix.dirname(file), crate, ".", ...(climbs ? [posix.dirname(crate)] : [])];
    const seen = new Set();
    for (const line of code) {
      for (const [, literal] of line.matchAll(/"([^"\\]*src[^"\\]*)"/g)) {
        const rel = literal.replace(/^\//, "");
        for (const at of places) {
          const path = posix.normalize(posix.join(at, rel));
          if (path !== "app/src" && !path.startsWith("app/src/")) continue;
          const folder = !tracked.has(path);
          if (folder && !folders.has(path) && path !== "app/src") continue;
          if (seen.has(path)) continue;
          seen.add(path);
          named.push({ file, path, folder });
        }
      }
    }
    const at = (re) => code.flatMap((line, i) => (re.test(line) ? [i] : []));
    const apps = at(/"app"/);
    const pieced = at(/"src"/).some((i) => apps.some((j) => Math.abs(i - j) <= 3));
    if (!seen.has("app/src") && pieced && SPLIT_READS.test(whole)) {
      named.push({ file, path: "app/src", folder: true });
    }
  }
  return named;
}
const rustNamesOfWindow = () => windowNamesIn(git("ls-files"), read);

test("the search finds the window's sources named by pieces, from any crate", () => {
  const files = {
    "app/src/a.ts": "",
    "app/src/ui/b.css": "",
    "app/src-tauri/Cargo.toml": "",
    "app/src-tauri/src/x.rs": [
      'let s = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("src");',
      "for e in std::fs::read_dir(s) {}",
    ].join("\n"),
    "crates/c/Cargo.toml": "",
    "crates/c/tests/y.rs": [
      'let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");',
      'walk(&root.join("app").join("src"));',
      "fn walk(d: &Path) { for e in std::fs::read_dir(d).unwrap() {} }",
    ].join("\n"),
    "crates/c/tests/z.rs":
      'const P: [&str; 2] = ["app", "src"]; let t = std::fs::read_to_string(p);',
    "crates/c/tests/only_names.rs": 'let a = "app"; let b = "src";',
    "app/src-tauri/src/own.rs": [
      'let s = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");',
      "for e in std::fs::read_dir(s) {}",
    ].join("\n"),
    "crates/c/tests/far.rs": [
      'let a = "app";',
      ...Array(5).fill("//"),
      "x();",
      "y();",
      "z();",
      "w();",
      'let b = dir.join("src"); std::fs::read_dir(b);',
    ].join("\n"),
    "operator/Cargo.toml": "",
    "operator/src/w.rs": 'include_str!("../../app/src/ui/b.css")',
    "tools/probe/src/v.rs": 'let p = "app/src/a.ts"; std::fs::read_to_string(p);',
  };
  const found = windowNamesIn(Object.keys(files), (p) => files[p]).map(
    ({ file, path, folder }) => `${file} ${folder ? "walks" : "reads"} ${path}`,
  );
  assert.deepEqual(found.sort(), [
    "app/src-tauri/src/x.rs walks app/src",
    "crates/c/tests/y.rs walks app/src",
    "crates/c/tests/z.rs walks app/src",
    "operator/src/w.rs reads app/src/ui/b.css",
    "tools/probe/src/v.rs reads app/src/a.ts",
  ]);
});

test("every file of the window's sources that Rust names is one Rust reads, or is guarded", () => {
  const named = rustNamesOfWindow();
  assert.ok(
    named.some((n) => n.path === "app/src/bindings.ts" && n.file === "app/src-tauri/src/lib.rs"),
    'the search finds concat!(env!("CARGO_MANIFEST_DIR"), "/../src/bindings.ts")',
  );
  assert.ok(
    named.some((n) => n.path === "app/src/ui/tabs/tabs.css"),
    'and include_str!("../../../../app/src/ui/tabs/tabs.css")',
  );
  for (const { file, path, folder } of named) {
    if (folder || GUARD_FILES.has(file) || file in NAMED_NOT_READ || file in WINDOW_WALK_MISSES) {
      continue;
    }
    assert.deepEqual(
      [...scopesOfPath(path, new Set())].sort(),
      all,
      `${file} reads ${path}: add it to READ_BY_RUST in tools/ci-scope.mjs`,
    );
  }
  const namers = new Set(named.map((n) => n.file));
  for (const file of Object.keys(NAMED_NOT_READ)) {
    assert.ok(namers.has(file), `${file} no longer names the window's sources: take it out`);
  }
  for (const path of READ_BY_RUST) {
    assert.ok(
      named.some((n) => n.path === path || (path.endsWith("/") && n.path.startsWith(path))),
      `${path} is no longer named by Rust: take it out of READ_BY_RUST`,
    );
  }
});

test("every Rust test that walks the window's sources is a guard, or a known miss", () => {
  const walkers = new Set(
    rustNamesOfWindow()
      .filter((n) => n.folder)
      .map((n) => n.file),
  );
  for (const guard of WINDOW_SOURCE_GUARDS) {
    assert.ok(walkers.has(guardFile(guard)), `${guardFile(guard)} is found walking app/src`);
  }
  for (const file of Object.keys(WINDOW_WALK_MISSES)) {
    assert.ok(walkers.has(file), `${file} no longer walks app/src: take it out`);
  }
  for (const file of walkers) {
    assert.ok(
      GUARD_FILES.has(file) || file in WINDOW_WALK_MISSES,
      `${file} walks the window's sources: add it to WINDOW_SOURCE_GUARDS in ` +
        "tools/ci-scope.mjs, or to WINDOW_WALK_MISSES here with why it cannot run there",
    );
  }
});

// 2026-10-31 (#3596): runner waits came back at 336 pull request runs a day, and 62 of the last 199
// merged pull requests changed Rust and nothing the window's checks read, yet ran them all.
const rustOnly = ["app", "docs", "rust", "scenario", "tools"];

test("a Rust change runs Rust, the app and the scenarios, not the web checks or the stories", () => {
  for (const path of [
    "crates/purlis-core/src/lib.rs",
    "crates/host/src/main.rs",
    "crates/purlis-core/tests/it/a_test.rs",
    "crates/purlis-core/docs/themes.md",
    "crates/host/Cargo.toml",
    "app/src-tauri/src/panels.rs",
  ]) {
    assert.deepEqual(of([path]), rustOnly, path);
  }
  assert.deepEqual(of(["crates/host/src/lib.rs", "changes/a-fix.md"]), rustOnly);
  assert.deepEqual(of(["crates/host/src/lib.rs", "app/src/App.tsx"]), all, "with the window");
  assert.deepEqual(of(["crates/host/src/lib.rs"], "merge_group"), all, "the queue runs it all");
});

test("the rest of the app's Rust folder, which the window's build reads, runs everything", () => {
  for (const path of [
    "app/src-tauri/Cargo.toml",
    "app/src-tauri/tauri.conf.json",
    "app/src-tauri/capabilities/default.json",
    "app/src-tauri/plugin/skills/theme/SKILL.md",
  ]) {
    assert.deepEqual(of([path]), all, path);
  }
});

test("Rust that the stories draw runs everything", () => {
  assert.ok(READ_BY_WEB.length > 0);
  for (const path of READ_BY_WEB) {
    const sample = path.endsWith("/") ? `${path}a.json` : path;
    assert.deepEqual(of([sample]), all, sample);
  }
});

// Every path under `crates/` or `app/src-tauri/` that a file of the window's checks names as a
// path: a string literal, or literals handed to one `join(..)`, read from the file's folder, from
// `app/` (`process.cwd()` in vitest) or from the root. A comment reads nothing.
function rustNamedByWindow(files, readFile) {
  const tracked = new Set(files);
  const isFolder = (p) => files.some((f) => f.startsWith(`${p}/`));
  // Keyed by path and file, so a file is found though another named the same path first.
  const named = new Map();
  const sources = files.filter(
    (f) =>
      /^app\/(src|e2e|\.storybook)\//.test(f) && /\.(ts|tsx|mts|js|mjs)$/.test(f),
  );
  for (const file of sources) {
    const text = readFile(file).replace(/\/\*[\s\S]*?\*\//g, "").replace(/^\s*\/\/.*$/gm, "");
    const literals = [...text.matchAll(/"[^"\n]*"(?:\s*,\s*"[^"\n]*")*/g)].map((match) =>
      [...match[0].matchAll(/"([^"\n]*)"/g)].map((m) => m[1]).join("/"),
    );
    // A template's tail, `${root}/crates/a/b.rs`: read from each folder as any literal is.
    const inTemplates = [...text.matchAll(/`[^`]*`/g)].flatMap((match) =>
      [...match[0].matchAll(/(?:crates|src-tauri)\/[A-Za-z0-9_./-]+/g)].map((m) => m[0]),
    );
    for (const joined of [...literals, ...inTemplates]) {
      for (const from of [posix.dirname(file), "app", "."]) {
        const path = posix.normalize(posix.join(from, joined)).replace(/\/$/, "");
        if (!/^(crates\/[^/]+\/|app\/src-tauri\/)/.test(path)) continue;
        if (!tracked.has(path) && !isFolder(path)) continue;
        named.set(`${path}\0${file}`, file);
      }
    }
  }
  return named;
}

test("the search finds a Rust path named whole, in pieces, or from a URL", () => {
  const files = [
    "app/src/a.test.ts",
    "app/e2e/b.test.ts",
    "app/.storybook/c.ts",
    "app/src/d.tsx",
    "app/src/e.test.ts",
    "crates/core/src/mark.rs",
    "crates/core/src/other.rs",
    "crates/core/src/theme/one.json",
    "app/src-tauri/src/opener.rs",
    "app/src-tauri/Cargo.toml",
  ];
  const text = {
    "app/src/a.test.ts": 'join(process.cwd(), "..", "crates", "core", "src", "mark.rs")',
    "app/e2e/b.test.ts": 'join(here, "..", "src-tauri", "src", "opener.rs")',
    "app/.storybook/c.ts": 'new URL("../../crates/core/src/theme", import.meta.url)',
    "app/src/d.tsx": '// join("..", "crates", "core", "src", "mark.rs")\nentry("crates", true)',
    "app/src/e.test.ts": "readFileSync(`${root}/crates/core/src/other.rs`)",
  };
  const found = [...rustNamedByWindow(files, (p) => text[p] ?? "").keys()]
    .map((key) => key.split("\0")[0])
    .sort();
  assert.deepEqual(found, [
    "app/src-tauri/src/opener.rs",
    "crates/core/src/mark.rs",
    "crates/core/src/other.rs",
    "crates/core/src/theme",
  ]);
});

// A file that names a Rust path but is no test of `npm test`, and what holds it.
const READERS_HELD_ELSEWHERE = {
  "app/.storybook/handed.plugin.ts": "the stories: READ_BY_WEB runs everything",
  "app/e2e/recordedForge.ts": "imported by recordedForge.test.ts, a reader, and a scenario spec",
};

test("every vitest file that names a Rust path is a reader, and every reader names one", () => {
  const readers = new Set(rustNamedByWindow(git("ls-files"), read).values());
  for (const file of readers) {
    assert.ok(
      file in READERS_HELD_ELSEWHERE || RUST_READERS.includes(file.slice("app/".length)),
      `${file} reads Rust: add it to RUST_READERS in tools/ci-scope.mjs`,
    );
  }
  for (const reader of RUST_READERS) {
    assert.ok(readers.has(`app/${reader}`), `app/${reader} reads no Rust: take it out`);
  }
  for (const file of Object.keys(READERS_HELD_ELSEWHERE)) {
    assert.ok(readers.has(file), `${file} reads no Rust: take it out`);
  }
});

test("the web job's part 2 runs every reader, when Rust changed and web is out of scope", () => {
  const ci = read(".github/workflows/ci.yml");
  const step = ci.slice(ci.indexOf("- name: Vitest's files that read Rust"));
  assert.ok(step.length < ci.length, "ci.yml has the step");
  const run = step.slice(0, step.indexOf("\n      - "));
  assert.match(
    run,
    /matrix\.part == 2 && needs\.changes\.outputs\.web == 'false' && needs\.changes\.outputs\.rust != 'false'/,
  );
  for (const reader of RUST_READERS) assert.ok(run.includes(reader), reader);
});

// `ci` never builds the operator: a workspace of its own, which `operator.yml` checks.
test("the operator alone runs the tools' tests, as `ci` never builds it", () => {
  for (const path of ["operator/src/main.rs", "operator/charts/a/values.yaml", "operator/Cargo.lock"]) {
    assert.deepEqual(of([path]), docsOnly, path);
  }
  assert.deepEqual(of(["operator/Dockerfile"]), all, "a Rust test reads it");
  const deleted = new Set(["operator/src/gone.rs"]);
  const scopes = scopesOf({ event: "pull_request", paths: [...deleted], ciToolNames: CI_TOOLS, deleted });
  assert.deepEqual([...scopes.scopes].sort(), docsOnly, "ci reads no path of it");
  assert.match(read(".github/workflows/operator.yml"), /- operator\/\*\*/);
});

test("every operator file a Rust test or `ci.yml` reads runs everything", () => {
  const rust = git("ls-files").filter((f) => f.endsWith(".rs") && !f.startsWith("operator/"));
  for (const file of rust) {
    const text = read(file).replace(/^\s*\/\/.*$/gm, "");
    for (const [, path] of text.matchAll(/"(operator\/[A-Za-z0-9._/-]+)"/g)) {
      if (path.includes("*") || path.endsWith("/")) continue;
      assert.deepEqual(of([path]), all, `${file} reads ${path}`);
    }
  }
});

test("web parts: the part that runs the tools' tests alone when the web checks are out", () => {
  assert.deepEqual(webParts(new Set(["docs", "tools"])), [2]);
  assert.deepEqual(webParts(new Set(rustOnly)), [2]);
  assert.deepEqual(webParts(new Set(SCOPES)), [1, 2, 3, 4]);
  const ci = read(".github/workflows/ci.yml");
  assert.ok(
    ci.includes("part: ${{ fromJSON(needs.changes.outputs.web-parts || '[1,2,3,4]') }}"),
    "an empty output runs every part",
  );
  assert.match(ci.slice(ci.indexOf("  web:\n")), /npm test -- --shard=\$\{\{ matrix\.part \}\}\/4/);
  const toolsStep = ci.slice(ci.indexOf('- run: node --test "../tools/*.test.mjs"'));
  assert.match(toolsStep.slice(0, 80), /if: matrix\.part == 2/, "part 2 has the tools' tests");
});

test("a job that only asks whether it is needed is not started on a pull request", () => {
  assert.deepEqual(
    gatesOf({ event: "pull_request", paths: ["docs/a.md"] }),
    {
      "listen-shim": false,
      "browser-engine": false,
      confinement: false,
      parity: false,
      bench: false,
      "outcome-bars": false,
    },
  );
  const yes = (path, gate) =>
    assert.equal(gatesOf({ event: "pull_request", paths: [path] })[gate], true, `${path}: ${gate}`);
  yes("tests/listen-shim/check.mjs", "listen-shim");
  yes("crates/purlis-cli/src/listen/mod.rs", "listen-shim");
  yes("crates/browser-engine/src/lib.rs", "browser-engine");
  yes("crates/studio-probe/src/lib.rs", "confinement");
  yes("crates/host/src/lib.rs", "parity");
  yes("crates/session-protocol/src/lib.rs", "bench");
  yes("Cargo.lock", "bench");
  for (const event of ["merge_group", "push", "schedule"]) {
    for (const [gate, on] of Object.entries(gatesOf({ event, paths: ["docs/a.md"] }))) {
      assert.equal(on, true, `${event}: ${gate}`);
    }
  }
  for (const on of Object.values(gatesOf({ event: "pull_request", paths: [] }))) {
    assert.equal(on, true, "a change of no paths starts every job");
  }
});

// By the operator's choice of 2026-11-01: what the bars measure and the suite itself. A regression
// from anywhere else shows on main's push, which always runs it.
test("the outcome bars run on a pull request that touches what they measure, and on main", () => {
  const on = (path) => gatesOf({ event: "pull_request", paths: [path] })["outcome-bars"];
  for (const path of [
    "app/src/SkillStudioTab.tsx",
    "app/src/UseSkill.tsx",
    "app/src/skillsListing.ts",
    "app/src/chatSkillsAsk.ts",
    "app/src/personaSkills.tsx",
    "app/src/SkillsChangedNotice.tsx",
    "app/src/ui/list/Fold.tsx",
    "app/src-tauri/src/skillstudio.rs",
    "app/src-tauri/src/skillstudio/needs.rs",
    "app/src-tauri/src/useskill.rs",
    "app/src-tauri/src/personaskills.rs",
    "crates/skills/src/lib.rs",
    "crates/purlis-core/src/skills.rs",
    "crates/purlis-core/src/skills/verbs.rs",
    "crates/purlis-core/src/skilluse.rs",
    "crates/fake-harness/src/main.rs",
    "app/e2e/outcomeBars.ts",
    "app/e2e/harness.ts",
    "app/e2e/wdio.outcome.conf.ts",
    "app/e2e/specs/skill-outcome-bars.outcome.e2e.ts",
    ".github/workflows/ci.yml",
  ])
    assert.equal(on(path), true, path);
  for (const path of ["app/src/App.tsx", "crates/host/src/lib.rs", "app/e2e/specs/tabs.e2e.ts"])
    assert.equal(on(path), false, path);

  const ci = read(".github/workflows/ci.yml");
  const job = ci.slice(ci.indexOf("\n  outcome-bars:\n"), ci.indexOf("\n  scenario-linux-checks:\n"));
  assert.ok(
    job.includes(
      "github.event_name == 'push' || (github.event_name == 'pull_request' && needs.changes.outputs.outcome-bars != 'false')",
    ),
    "main's push and a pull request its gate lets in, and not the merge group",
  );
  assert.match(
    ci.slice(ci.indexOf("\n  changes:\n"), ci.indexOf("\n  rust:\n")),
    /\boutcome-bars: \$\{\{ steps\.scope\.outputs\.outcome-bars \}\}/,
  );
});

// A gate says what the job's own step decides: the step stays, for the merge group and the push,
// which are judged in the job as they always were.
test("each gated job reads its gate, and its own step greps the same paths", () => {
  const ci = read(".github/workflows/ci.yml");
  const jobs = {
    "listen-shim": "listen-shim",
    "browser-engine": "browser-engine-macos",
    confinement: "studio-confinement-macos",
    parity: "parity",
    bench: "bench",
  };
  for (const [gate, job] of Object.entries(jobs)) {
    const from = ci.indexOf(`\n  ${job}:\n`);
    assert.ok(from > 0, job);
    const body = ci.slice(from + 1);
    const end = body.slice(1).search(/\n  [a-z][a-z0-9-]*:\n/);
    const text = body.slice(0, end + 1);
    assert.match(text, /\n    needs: changes\n/, `${job} needs changes`);
    assert.ok(
      text.includes(`github.event_name != 'pull_request' || needs.changes.outputs.${gate} != 'false'`),
      `${job} is started unless its gate says false on a pull request`,
    );
    assert.ok(text.includes(GATES[gate]), `${job}'s step greps GATES["${gate}"]`);
    assert.match(
      ci.slice(ci.indexOf("\n  changes:\n"), ci.indexOf("\n  rust:\n")),
      new RegExp(`\\b${gate}: \\$\\{\\{ steps\\.scope\\.outputs\\.${gate} \\}\\}`),
      `changes hands on ${gate}`,
    );
  }
});

test("a change that cannot be read runs every scope, part and gated job", () => {
  const dir = mkdtempSync(join(tmpdir(), "ci-scope-"));
  const out = join(dir, "out");
  execFileSync("node", [join(ROOT, "tools/ci-scope.mjs"), "pull_request", "no-such-commit"], {
    cwd: ROOT,
    env: { ...process.env, GITHUB_OUTPUT: out },
    stdio: "pipe",
  });
  const said = Object.fromEntries(
    readFileSync(out, "utf8").trim().split("\n").map((l) => l.split("=")),
  );
  for (const key of [...SCOPES, ...Object.keys(GATES)]) assert.equal(said[key], "true", key);
  assert.equal(said["web-parts"], "[1,2,3,4]");
  rmSync(dir, { recursive: true });
});
