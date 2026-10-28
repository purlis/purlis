// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The scopes CI's heavy jobs run by (#3505): sample paths to scopes, and the rules held to the
// repository as it is. Run with `node --test tools/`.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { posix } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import {
  AGENT_STEERING,
  READ_BY_CODE,
  READ_BY_RUST,
  SCOPES,
  WHOLE_TREE_GUARDS,
  WINDOW_SOURCE_GUARDS,
  ciToolsOf,
  namedIn,
  scopesOf,
  scopesOfPath,
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
    "crates/purlis-core/src/lib.rs",
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
// `join("..", "docs", "x.md")`, or a root Markdown file the same ways. A comment's link to a doc
// reads nothing, so comment lines are left out.
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
  assert.ok(read.has("CHANGELOG.md"), 'the search finds include_str!("../../../CHANGELOG.md")');
  assert.ok(read.has("docs/design-system.md"), 'and join(.., "docs", ..)');
  for (const path of read) {
    if (!tracked.has(path)) continue;
    assert.deepEqual(
      [...scopesOfPath(path, new Set())].sort(),
      all,
      `${path} is read by code: add it to READ_BY_CODE in tools/ci-scope.mjs`,
    );
  }
  assert.ok(READ_BY_CODE.length > 0);
});

// A file of code or tests that holds the repository's root: it names a place it is anchored at
// (`CARGO_MANIFEST_DIR`, `import.meta.url`, `process.cwd()`) and climbs from it to the root
// itself, not into a folder below it: `"../.."` or `"../../"` (a `.join` on the next line, a
// `resolve(…, "../..")`, a root held in a variable), `"..", ".."` with nothing after it,
// `process.cwd(), ".."` from `app/`, or `import.meta.glob("../../*…")`.
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
  "crates/purlis-core/tests/every_git_call_goes_through_the_runner.rs":
    "crates/ and app/src-tauri/",
  "crates/purlis-core/tests/four_unsafe_blocks.rs": "crates/ and app/src-tauri/ Rust",
  "crates/purlis-core/tests/only_the_resolver_builds_the_native_transport.rs": "shipped Rust",
  "crates/purlis-core/tests/only_the_updater_and_report_name_charters_own_address.rs":
    "shipped Rust",
  "crates/purlis-core/tests/shipped_literals/mod.rs": "crates/ and app/src-tauri/ Rust",
  "crates/purlis-core/tests/the_local_layer_has_one_reader.rs": "crates/ and app/src-tauri/ Rust",
};

test("a test that walks the repository from its root is a whole-tree guard or walks code", () => {
  const walkers = rootedFiles().filter((p) => WALKS.test(read(p)));
  const guards = GUARD_FILES;
  for (const { crate, test: name } of WHOLE_TREE_GUARDS) {
    const path = `crates/${crate}/tests/${name}.rs`;
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
    assert.ok(run.includes(`-p ${crate}`), crate);
    assert.ok(run.includes(`--test ${name}`), name);
  }
});

// Every path under `app/src/` that Rust names (#3596): a string in a Rust file of the workspace
// that, read from the file's folder (`include_str!`), its crate's folder (`CARGO_MANIFEST_DIR`)
// or the repository's root (`root.join(..)`), is a tracked file or folder of the window's
// sources. A file named is read by Rust: it is in `READ_BY_RUST`, or the test naming it is a guard
// `recorded` runs, or it is named below with why it is not read. A folder named is walked: the
// test is a guard, or it is a known miss below. Comment lines read nothing.
const guardFile = ({ crate, test: name }) => `crates/${crate}/tests/${name}.rs`;
const GUARD_FILES = new Set([...WHOLE_TREE_GUARDS, ...WINDOW_SOURCE_GUARDS].map(guardFile));

// A Rust file that names a file of the window's sources without reading it, and why.
const NAMED_NOT_READ = {
  "crates/purlis-core/src/files/find.rs": "a fuzzy match's sample path, matched as text",
  "crates/purlis-core/tests/a_file_is_found_by_fuzzy_name.rs":
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
