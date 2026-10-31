// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// What a change touches, as the scopes CI's heavy jobs are run by (#3505): the one place that
// says which paths need which jobs. `ci.yml`'s `changes` job and `stories.yml`'s run it, through
// `.github/actions/change-scope`, and every heavy job reads its outputs; no heavy job carries a
// path pattern of its own.
//
// The scopes:
//
// - `rust`: the Rust suite in its parts, fmt and clippy, the licences and advisories, the
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
// 2. `docs/`, `site/`, `changes/`, and `AGENTS.md` and `CLAUDE.md` (an agent's steering, which no
//    test reads): the docs alone. Every other file at the root runs everything: tests read them
//    by name (`community.test.ts` holds `README.md`, `CONTRIBUTING.md`, `SUPPORT.md`,
//    `CODE_OF_CONDUCT.md` and the `SECURITY.md` they link to; `app/src-tauri` reads `LICENSE`
//    and `NOTICE`), and a manifest or a lockfile is read by every build. A file deleted from
//    any of these runs everything too: a root file a test holds may link to it
//    (`CONTRIBUTING.md` links `changes/README.md`, #3520).
// 3. A file under `tools/` that CI runs (`ciTools`): every scope. So is a file deleted from
//    `tools/` (a rename is a deletion and an addition): what named it may name it still, and a
//    deleted file can no longer be read for what it names. Any other file there, a tool a person
//    runs by hand or a tool's test: the tools' tests alone.
// 4. `app/src/`, the window's sources: every scope but `rust` and `docs` (#3596). No Rust build
//    reads them, and the Rust tests that do are either named by the files they read
//    (`READ_BY_RUST`: a change to one of those runs everything) or walk the whole folder and run
//    on every change (`WINDOW_SOURCE_GUARDS`). One walker is left to the merge group, which runs
//    every scope: a unit test of the app crate, whose test build needs Tauri
//    (`WINDOW_WALK_MISSES` in the test). `app/e2e/`: the web checks (its type check and lint)
//    and the scenario specs.
// 5. `crates/` and `app/src-tauri/src/`, the Rust: every scope but `web` and `stories` (#3596,
//    2026-10-31). The app is built from it and the scenarios drive it. Vitest's files that read
//    it run all the same (`RUST_READERS`), alone, and a story draws only the built-in themes
//    (`READ_BY_WEB`, which runs everything). A Rust change that moves what the window calls
//    changes `app/src/bindings.ts` too, which runs everything.
// 6. `operator/`: the docs and tools alone. `ci` never builds it, a workspace of its own that
//    `operator.yml` checks, but for `operator/Dockerfile`, which a Rust test reads.
// 7. Anything else: every scope. An unknown path runs everything, so a path this file has never
//    heard of costs time, never a check.
//
// **The whole-tree guards run on every change** (`WHOLE_TREE_GUARDS`): Rust tests that read every
// file of the repository, so a change to docs or tools can fail them (a source file with no
// licence header, a file a sandboxed chat may not write). When `rust` is out of scope, the
// `recorded` job runs these alone. `ci-scope.test.mjs` finds each test that walks from the
// repository's root and fails until it is listed here or named as walking only code.
//
// **A job that only asks whether it is needed is not started on a pull request** (`GATES`, #3596).
// `listen-shim`'s 14 legs, the two macOS jobs, `parity` and `bench` each take a runner to read the
// diff and usually end there: 18 of a pull request's 38 jobs by 2026-10-31. They are not required
// checks, so `changes` answers for them, and a skipped job passes. Their own step stays and decides
// in a merge group and on a push, as it always has.
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
// The window's sources (`app/src/`) are built into the app and drawn by the scenarios and the
// stories, but no Rust build or test reads them, except the files in `READ_BY_RUST` and the
// guards in `WINDOW_SOURCE_GUARDS` (#3596).
const WINDOW = new Set(["web", "app", "scenario", "stories", "tools"]);
// The Rust: built into the app and driven by the scenarios, read by no web check or story but for
// `READ_BY_WEB`. `docs` for the record, as every scope but the window's says it (#3596).
const RUST = new Set(["rust", "app", "scenario", "tools", "docs"]);

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

// The files of the window's sources that Rust reads by name, so a change to one is a change to a
// Rust build or test: the generated bindings and RPC table (held to what the Rust makes of them),
// the theme module and stylesheets the theme engine's tests hold its parts to, and the data two
// app tests read. `ci-scope.test.mjs` finds each `app/src/` path the Rust names and fails when
// one is missing here. A folder ends in `/`.
export const READ_BY_RUST = [
  "app/src/bindings.ts",
  "app/src/uiRpc.ts",
  "app/src/writeSites.json",
  "app/src/invisible.cases.json",
  "app/src/theme/theme.ts",
  "app/src/theme/geometry.css",
  "app/src/App.css",
  "app/src/ui/activity/activity.css",
  "app/src/ui/badge/badge.css",
  "app/src/ui/button/button.css",
  "app/src/ui/card/card.css",
  "app/src/ui/choice/choice.css",
  "app/src/ui/dialog/dialog.css",
  "app/src/ui/disclosure/disclosure.css",
  "app/src/ui/empty-state/empty-state.css",
  "app/src/ui/field/field.css",
  "app/src/ui/list/list.css",
  "app/src/ui/menu/menu.css",
  "app/src/ui/meter/meter.css",
  "app/src/ui/notice/notice.css",
  "app/src/ui/panel/panel.css",
  "app/src/ui/popover/popover.css",
  "app/src/ui/rail/rail.css",
  "app/src/ui/review-page/review-page.css",
  "app/src/ui/select/select.css",
  "app/src/ui/separator/separator.css",
  "app/src/ui/status/status.css",
  "app/src/ui/table/table.css",
  "app/src/ui/tabs/tabs.css",
  "app/src/ui/titlebar/titlebar.css",
  "app/src/ui/tooltip/tooltip.css",
  "app/src/ui/tree/tree.css",
];

// The Rust the stories draw, so a change to it runs every scope: the built-in themes, which
// `.storybook/handed.plugin.ts` hands to each story. A folder ends in `/`.
export const READ_BY_WEB = ["crates/purlis-core/src/theme/built-in/"];

// Vitest's files (from `app/`) that read Rust: `copy.rust.test.ts` holds the window's copy written
// in every Rust file, and the others hold a constant or a theme. `ci.yml`'s `web` job runs them
// alone when Rust changed and `web` is out of scope. `ci-scope.test.mjs` finds each file that names
// a Rust path and fails when one is missing here, or names none now.
export const RUST_READERS = [
  "e2e/dialogs.test.ts",
  "e2e/fence.test.ts",
  "e2e/gitAnswer.test.ts",
  "e2e/recordedForge.test.ts",
  "e2e/standInForge.test.ts",
  "e2e/upgrade.test.ts",
  "src/PersonaMark.test.tsx",
  "src/StripMarks.test.tsx",
  "src/allowLabels.test.ts",
  "src/copy.rust.test.ts",
  "src/layoutProjects.test.ts",
  "src/settings/linkedGroups.test.ts",
  "src/specimen/Specimen.test.tsx",
  "src/theme/consentToggles.test.ts",
  "src/theme/contrast.test.ts",
  "src/theme/core.test.ts",
  "src/theme/forced-colors.test.ts",
  "src/theme/grounds.test.ts",
  "src/theme/railIcons.vendor.test.ts",
  "src/theme/registry.test.ts",
  "src/theme/theme.test.ts",
  "src/theme/themeIds.test.ts",
  "src/theme/tint.test.ts",
  "src/ui/buildingSkill.test.ts",
  "src/ui/field/refusingFields.guard.test.ts",
  "src/ui/menu/submenuRows.guard.test.ts",
  "src/ui/notice/noticeCardFill.test.ts",
  "src/windowprefs.test.ts",
];

// Rust tests read this operator file by name; the rest of `operator/` no `ci` job reads.
const OPERATOR_READ_BY_RUST = new Set(["operator/Dockerfile"]);

// The paths each job greps for in its own step, word for word (`ci-scope.test.mjs` holds them
// together): what `gatesOf` answers before a runner is taken for it.
export const GATES = {
  "listen-shim":
    "^(crates/purlis-cli/src/listen|tests/listen-shim/|\\.github/workflows/ci\\.yml$)",
  "browser-engine":
    "^(crates/browser-engine/|Cargo\\.lock$|rust-toolchain\\.toml$)",
  "confinement":
    "^(crates/studio-probe/|\\.github/actions/studio-probe-names/|crates/purlis-core/src/(executor|extension)(\\.rs|/)|crates/purlis-core/src/sandbox/(seatbelt|confined|egress|reach|wrapper|hosts|localport|door)(\\.rs|/)|crates/purlis-core/src/hookwire/serve(\\.rs|/)|crates/purlis-cli/src/listen/|crates/purlis-core/tests/it/a_sandboxed_chat_listens_nowhere_under_seatbelt\\.rs|Cargo\\.lock$|rust-toolchain\\.toml$)",
  "parity":
    "^(crates/(host|session-protocol|linux-sandbox|browser-engine|fake-harness|terminal|stand-in)/|crates/purlis-core/src/(sandbox|browser|hook|runner|eventlog|linkkey|host|dockerpool|pools|codelink|session)[^/]*(\\.rs$|/)|crates/purlis-cli/src/(bridge|mcp|hooks|browsing|main)[^/]*(\\.rs$|/)|app/src-tauri/plugin/|app/src-tauri/src/(behaviour|host|session|hooks|killswitch|browser|chatbrowsers|routedsessions|runner|dockerpools|sandboxing|poolfollow)[^/]*(\\.rs$|/)|runner-image/|tests/parity/|Cargo\\.lock$|rust-toolchain\\.toml$|\\.github/workflows/ci\\.yml$)",
  "bench":
    "^(crates/session-protocol/|Cargo\\.toml$|Cargo\\.lock$|rust-toolchain\\.toml$|fixtures/corpora/|tools/bench\\.mjs$|tools/latency-gate\\.mjs$|\\.github/workflows/ci\\.yml$)",
};

// The Rust tests that read every file of the window's sources (#3596): `ci.yml`'s `recorded`
// job runs each one when `rust` is out of scope, beside the whole-tree guards, so a change to
// the window that skips the Rust suite is still held to them. They link only the core. Here and
// in `WHOLE_TREE_GUARDS`, `test` is a module of the crate's one test binary, `it`
// (`tests/it/<test>.rs`, #3613), which the job picks by name.
export const WINDOW_SOURCE_GUARDS = [
  { crate: "purlis-core", test: "no_old_name_is_spelled_in_the_window_or_its_assets" },
  { crate: "purlis-core", test: "every_deferral_names_its_ticket" },
];

// The Rust tests that read every file of the repository: `ci.yml`'s `recorded` job runs each one
// when `rust` is out of scope, so no change skips them.
export const WHOLE_TREE_GUARDS = [
  { crate: "purlis-core", test: "every_first_party_source_file_carries_its_licence_header" },
  { crate: "purlis-core", test: "no_tracked_file_is_one_a_sandboxed_chat_may_not_write" },
];

// The files at the root a change to which runs the docs alone: what an agent reads to steer
// itself, which no test reads. Every other root file runs everything.
export const AGENT_STEERING = new Set(["AGENTS.md", "CLAUDE.md"]);

const listed = (list, path) =>
  list.some((entry) => (entry.endsWith("/") ? path.startsWith(entry) : path === entry));
const readByCode = (path) => listed(READ_BY_CODE, path);

// The scopes one changed path needs. `ciTools` is the set of file names under `tools/` that CI
// runs (`ciTools()` below).
export function scopesOfPath(path, ciToolNames) {
  if (path.startsWith(".github/") || SHARED.has(path) || readByCode(path)) return ALL;
  if (path.startsWith("app/src/")) return listed(READ_BY_RUST, path) ? ALL : WINDOW;
  if (path.startsWith("crates/") || path.startsWith("app/src-tauri/src/")) {
    return listed(READ_BY_WEB, path) ? ALL : RUST;
  }
  if (path.startsWith("operator/")) return OPERATOR_READ_BY_RUST.has(path) ? ALL : DOCS;
  if (path.startsWith("app/") && !path.startsWith("app/e2e/")) {
    // The app's manifests and shared configs (`package.json`, the lockfile, `tsconfig*.json`,
    // `vite.config.ts`, eslint and prettier), its `src-tauri`, icons, pages and `.storybook`.
    return ALL;
  }
  if (/^(docs|site|changes)\//.test(path) || AGENT_STEERING.has(path)) return DOCS;
  if (path.startsWith("tools/")) {
    const name = path.slice("tools/".length);
    // A folder of tools' data (`ucd/`, `licence-texts/`) or a probe of its own is CI's.
    if (name.includes("/")) return ALL;
    return ciToolNames.has(name) ? ALL : TOOLS;
  }
  if (path.startsWith("app/e2e/")) return E2E;
  return ALL;
}

// A path whose deletion runs everything though its edit runs less: a file under `tools/`, which
// what named it may name still, and a file that runs the docs alone (`docs/`, `site/`,
// `changes/`, an agent's steering), which a root file a test holds may link to:
// `CONTRIBUTING.md` links `changes/README.md`, and `community.test.ts` fails on a dead link
// (#3520). A rename is a deletion and an addition (`--no-renames`), so it counts too.
const narrowWhenDeleted = (path) =>
  /^(tools|docs|site|changes)\//.test(path) || AGENT_STEERING.has(path);

// The scopes a change needs: the union of its paths', and every scope for an event that is not a
// pull request or a change of no paths. `deleted` is the paths the change deletes.
export function scopesOf({ event, paths, ciToolNames, deleted = new Set() }) {
  if (event !== "pull_request" || paths.length === 0) return { scopes: ALL, why: [] };
  const scopes = new Set();
  const why = [];
  for (const path of paths) {
    const needs =
      deleted.has(path) && narrowWhenDeleted(path) ? ALL : scopesOfPath(path, ciToolNames);
    if (needs === ALL) why.push(path);
    for (const scope of needs) scopes.add(scope);
  }
  return { scopes, why };
}

// Whether each gated job is needed. Only a pull request is answered; every other event, and a
// change of no paths, starts the job, whose own step then decides.
export function gatesOf({ event, paths }) {
  const open = event !== "pull_request" || paths.length === 0;
  return Object.fromEntries(
    Object.entries(GATES).map(([gate, pattern]) => {
      const re = new RegExp(pattern);
      return [gate, open || paths.some((path) => re.test(path))];
    }),
  );
}

// Which parts of `web` a change needs: part 2 holds the tools' tests, which run on every change.
// The other three would each take a runner to say there is nothing to do.
export const webParts = (scopes) => (scopes.has("web") ? [1, 2, 3, 4] : [2]);

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
  let gates;
  try {
    let paths = [];
    let ciToolNames = new Set();
    const deleted = new Set();
    if (event === "pull_request") {
      const git = (...args) =>
        execFileSync("git", args, { encoding: "utf8", maxBuffer: 1 << 28 }).split("\n");
      // `--no-renames`, so a rename is both of its paths: moving a file out of `crates/` is a
      // change to `crates/`.
      for (const line of git("diff", "--name-status", "--no-renames", base, "HEAD")) {
        const [status, path] = line.split("\t");
        if (!path) continue;
        paths.push(path);
        if (status === "D") deleted.add(path);
      }
      const files = git("ls-files").filter(Boolean);
      ciToolNames = ciToolsOf(files, (p) => {
        try {
          return readFileSync(p, "utf8");
        } catch {
          return "";
        }
      });
    }
    result = scopesOf({ event, paths, ciToolNames, deleted });
    gates = gatesOf({ event, paths });
    console.log(paths.join("\n"));
  } catch (error) {
    console.log(`::warning::the change could not be read, so every job runs: ${error.message}`);
    result = { scopes: ALL, why: [] };
    gates = gatesOf({ event, paths: [] });
  }
  for (const scope of SCOPES) say(`${scope}=${result.scopes.has(scope)}`);
  for (const [gate, on] of Object.entries(gates)) say(`${gate}=${on}`);
  say(`web-parts=${JSON.stringify(webParts(result.scopes))}`);
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
