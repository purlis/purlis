// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// What a change touches, as the scopes CI's heavy jobs are run by (#3505): the one place that
// says which paths need which jobs. `ci.yml`'s `changes` job and `stories.yml`'s run it, through
// `.github/actions/change-scope`, and every heavy job reads its outputs; no heavy job carries a
// path pattern of its own.
//
// The scopes:
//
// - `rust`: the Rust suite in two parts, fmt and clippy, the licences and advisories, the
//   recorded behaviour;
// - `web`: the type check, lint, format check, the app's npm licences and vitest;
// - `app`: the app built as it ships, on Linux and macOS, and the Linux cold start;
// - `scenario`: the scenario specs on macOS and Linux;
// - `stories`: every component story, in Chromium and WebKit (`stories.yml`);
// - `tools`: the tools' own tests (`node --test tools/*.test.mjs`). In every scope set: they take
//   seconds, and several read what other changes touch (`docs/adr/`, the workflows);
// - `docs`: the docs site. Reported for the record: `docs.yml` builds the site on every change.
//
// **What a path needs, in order** (`scopesOfPath`):
//
// 1. A file under `.github/`, a lockfile, the toolchain, a manifest or a shared config: every
//    scope. So is a doc that code reads (`READ_BY_CODE`): it is compiled into a build or a test
//    holds it.
// 2. `docs/`, `site/`, `changes/` and a Markdown file at the root: the docs alone.
// 3. A file under `tools/` that CI runs (`ciTools`): every scope. Any other file there, a tool a
//    person runs by hand or a tool's test: the tools' tests alone.
// 4. `app/e2e/`: the web checks (its type check and lint) and the scenario specs.
// 5. Anything else: every scope. An unknown path runs everything, so a path this file has never
//    heard of costs time, never a check.
//
// **Only a pull request is scoped.** A merge group, a push to main, a nightly and a run by hand
// run every scope: the merge group is the last check before main, and it measures the pull
// request against its siblings ahead of it, so a scope rule that is wrong costs a pull request
// one trip through the queue and never a red main.
//
// Run as `node tools/ci-scope.mjs <event> <base>` in a checkout whose HEAD is the change: it
// writes `<scope>=true|false` for each scope to `$GITHUB_OUTPUT` (or stdout), and says on the
// run which paths decided it. Any failure to read the change runs every scope.

import { execFileSync } from "node:child_process";
import { appendFileSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

export const SCOPES = ["rust", "web", "app", "scenario", "stories", "tools", "docs"];

const ALL = new Set(SCOPES);
const DOCS = new Set(["docs", "tools"]);
const TOOLS = new Set(["tools"]);
const E2E = new Set(["web", "scenario", "tools"]);

// Changing one of these changes what every job builds or how it runs.
const SHARED = new Set([
  "Cargo.toml",
  "Cargo.lock",
  "rust-toolchain.toml",
  ".nvmrc",
  "deny.toml",
  "npm-licences.json",
]);

// Docs that code reads, so a change to one is a change to a build or a test: `CHANGELOG.md` is
// compiled into the app and the CLI (`about.rs`, `news.rs`), and each other one is read by a test
// (`include_str!`, or `readFileSync` in vitest). `ci-scope.test.mjs` finds each doc the code
// names by a relative path and fails when one is missing here.
export const READ_BY_CODE = [
  "CHANGELOG.md",
  "docs/plane-format.md",
  "docs/schema/",
  "docs/ui-primitives.md",
  "docs/design-system.md",
  "docs/adr/0076-a-run-moves-only-by-a-named-cause-and-a-chats-state-is-read-from-its-runs.md",
];

const readByCode = (path) =>
  READ_BY_CODE.some((doc) => (doc.endsWith("/") ? path.startsWith(doc) : path === doc));

// The scopes one changed path needs. `ciTools` is the set of file names under `tools/` that CI
// runs (`ciTools()` below).
export function scopesOfPath(path, ciToolNames) {
  if (path.startsWith(".github/") || SHARED.has(path) || readByCode(path)) return ALL;
  if (path.startsWith("app/") && !path.startsWith("app/src/") && !path.startsWith("app/e2e/")) {
    // The app's manifests and shared configs (`package.json`, the lockfile, `tsconfig*.json`,
    // `vite.config.ts`, eslint and prettier), its `src-tauri`, icons, pages and `.storybook`.
    return ALL;
  }
  if (/^(docs|site|changes)\//.test(path) || /^[^/]+\.md$/.test(path)) return DOCS;
  if (path.startsWith("tools/")) {
    const name = path.slice("tools/".length);
    // A folder of tools' data (`ucd/`, `licence-texts/`) or a probe of its own is CI's.
    if (name.includes("/")) return ALL;
    return ciToolNames.has(name) ? ALL : TOOLS;
  }
  if (path.startsWith("app/e2e/")) return E2E;
  return ALL;
}

// The scopes a change needs: the union of its paths', and every scope for an event that is not a
// pull request or a change of no paths.
export function scopesOf({ event, paths, ciToolNames }) {
  if (event !== "pull_request" || paths.length === 0) return { scopes: ALL, why: [] };
  const scopes = new Set();
  const why = [];
  for (const path of paths) {
    const needs = scopesOfPath(path, ciToolNames);
    if (needs === ALL) why.push(path);
    for (const scope of needs) scopes.add(scope);
  }
  return { scopes, why };
}

// The file names under `tools/` that CI runs: each one a file outside `tools/` and the docs
// names (`tools/<name>`: a workflow, an action, `app/package.json`, `vite.config.ts`, a test),
// and each one such a tool names in turn, its tests aside. A mention in a comment counts too,
// which only ever runs more. `files` is every tracked path, and `read` reads one.
export function ciToolsOf(files, read) {
  const names = new Set(files.filter((p) => /^tools\/[^/]+$/.test(p)).map((p) => p.slice(6)));
  const outside = files.filter(
    (p) => !/^(tools|docs|site|changes)\//.test(p) && !p.endsWith(".md") && !isBinary(p),
  );
  const ci = new Set();
  for (const path of outside) {
    for (const [, name] of read(path).matchAll(/tools\/([A-Za-z0-9._-]+)/g)) {
      if (names.has(name)) ci.add(name);
    }
  }
  const queue = [...ci];
  while (queue.length > 0) {
    const text = read(`tools/${queue.pop()}`);
    for (const name of names) {
      // A tool's test is run by `node --test`, never by a tool that names it.
      if (!ci.has(name) && !name.endsWith(".test.mjs") && namedIn(text, name)) {
        ci.add(name);
        queue.push(name);
      }
    }
  }
  return ci;
}

// Whether `text` names `name` as a whole file name: not inside a longer one, so `bench.mjs` is
// not named by `stress-bench.mjs`. A plain scan, not a pattern built from the name.
const NAME_CHAR = /[A-Za-z0-9_-]/;
export function namedIn(text, name) {
  for (let at = text.indexOf(name); at !== -1; at = text.indexOf(name, at + 1)) {
    const before = at === 0 ? "" : text[at - 1];
    const after = text[at + name.length] ?? "";
    if (!NAME_CHAR.test(before) && !NAME_CHAR.test(after)) return true;
  }
  return false;
}

const isBinary = (path) =>
  /\.(png|jpe?g|gif|ico|icns|webp|woff2?|ttf|otf|gz|zip|wasm|pdf|mp4|bin)$/i.test(path);

function main([event, base]) {
  const out = process.env.GITHUB_OUTPUT;
  const say = (line) => (out ? appendFileSync(out, `${line}\n`) : console.log(line));
  let result;
  try {
    let paths = [];
    let ciToolNames = new Set();
    if (event === "pull_request") {
      const git = (...args) =>
        execFileSync("git", args, { encoding: "utf8", maxBuffer: 1 << 28 }).split("\n");
      // `--no-renames`, so a rename is both of its paths: moving a file out of `crates/` is a
      // change to `crates/`.
      paths = git("diff", "--name-only", "--no-renames", base, "HEAD").filter(Boolean);
      const files = git("ls-files").filter(Boolean);
      ciToolNames = ciToolsOf(files, (p) => {
        try {
          return readFileSync(p, "utf8");
        } catch {
          return "";
        }
      });
    }
    result = scopesOf({ event, paths, ciToolNames });
    console.log(paths.join("\n"));
  } catch (error) {
    console.log(`::warning::the change could not be read, so every job runs: ${error.message}`);
    result = { scopes: ALL, why: [] };
  }
  for (const scope of SCOPES) say(`${scope}=${result.scopes.has(scope)}`);
  const skipped = SCOPES.filter((s) => !result.scopes.has(s));
  if (skipped.length === 0) {
    const because = result.why.length > 0 ? `: ${result.why.slice(0, 5).join(", ")}` : "";
    console.log(`::notice title=Scope::every job runs (${event}${because})`);
  } else {
    const runs = SCOPES.filter((s) => result.scopes.has(s));
    console.log(`::notice title=Scope::runs ${runs.join(", ")}; skips ${skipped.join(", ")}`);
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main(process.argv.slice(2));
