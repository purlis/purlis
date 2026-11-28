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
//    A doc only Rust's tests read (`DOCS_READ_BY_RUST`) needs what the Rust needs (5. below).
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
// 6. `operator/`: the docs and tools alone. A workspace of its own that `operator.yml` checks,
//    but for `operator/Dockerfile`, which a Rust test reads. `ci` builds it only for Pool setup's
//    walk on a kind cluster (#4494), which its gate (`GATES`) starts.
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
// **`ships`: whether a dev build has anything to build** (#7383, R4). `release.yml`'s `plan` runs
// `node tools/ci-scope.mjs ships <kind> <sha>` for a push to `main` and skips the build when
// nothing that reaches a build changed. The change is measured from the commit the published
// `dev.json` names, never from the push's parent: a pending run can be displaced by the next
// push, and a crate change in the displaced commit must not be skipped because the commit on top
// of it is docs. Not shipped: docs, research, Markdown other than `CHANGELOG.md` (compiled into
// the app), test-only files and workflows other than `release.yml`. Everything else ships,
// including a path nothing here knows. A stable tag, a hand-started build and the nightly always
// build; and so does any change that cannot be read (no `dev.json`, a compare that says nothing
// of order, one too long to list). The skip only means no new build: nothing is published,
// signed or replaced (ADR 0042).
//
// **Per platform** (#7324): `ships` also says `macos` and `linux`. A change that only Linux reads
// (`LINUX_ONLY`) would skip the macOS build, but that is held OFF by `PARTIAL_DEV_BUILDS`: one
// `dev.json` names one version for both platforms, so a Linux-only publish would pair the new
// version with the previous macOS archive. While it is off, `macos` is `ships`.
//
// **The ready gate** (#7335, ADR 0127): a draft pull request runs the fast guards and the web tests
// and nothing heavy. `HEAVY` (`rust`, `app`, `scenario`, `stories`) and every gated job are off on
// a draft, whatever its paths say, a change to `.github/` included: marking the draft ready
// (`ready_for_review`, which `ci.yml` and `stories.yml` trigger on) runs the change's scopes
// without another push, and a change to the workflows turns every one of them on then. A merge
// group, a push and a nightly run every scope and both story browsers, so a pull request run is
// a subset of the queue run (`ci-stages.test.mjs` holds the workflows to it).
//
// Run as `node tools/ci-scope.mjs <event> <base> [draft] [labels]` in a checkout whose HEAD is the
// change (`draft` is `true` for a draft pull request, `labels` its label names joined by commas,
// which choose the story browsers): it writes `<scope>=true|false` for each scope to
// `$GITHUB_OUTPUT` (or stdout), and says on the run which paths decided it. Any failure to read
// the change runs every scope.

import { execFileSync } from "node:child_process";
import { appendFileSync, readFileSync } from "node:fs";
import { matchesGlob } from "node:path";
import { fileURLToPath } from "node:url";

import { readPublished } from "./dev-build-order.mjs";

export const SCOPES = ["rust", "web", "app", "scenario", "stories", "tools", "docs"];

// The scopes a draft pull request does not run: each takes a runner for minutes, and a ready PR,
// the queue and main run them (#7335).
export const HEAVY = ["rust", "app", "scenario", "stories"];

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
  "docs/ui-primitives.md",
  "docs/design-system.md",
  // `app/src/ui/buildingDoc.test.ts` holds it to the code (#5725).
  "docs/building-ui.md",
  "docs/adr/0076-a-run-moves-only-by-a-named-cause-and-a-chats-state-is-read-from-its-runs.md",
  // The published theme schema: Rust writes and checks it, and the title bar's test reads every
  // part a theme can set from it (#3196).
  "docs/schema/",
];

// Docs that Rust's tests read and no window file does, so a change to one needs what a change to
// Rust needs: `docs/plane-format.md` was in 35 of the last 199 merged pull requests, most of them
// otherwise Rust alone (#3596). ADR 0083's amendment on a task question's counts is the table
// `eventlog::questions`'s test holds a question's steps to (#4634). `ci-scope.test.mjs` fails
// when a window file comes to read one.
export const DOCS_READ_BY_RUST = [
  "docs/plane-format.md",
  "docs/adr/0083/2026-11-02-a-task-question-is-counted-from-its-steps-and-never-read.md",
  // `it/every_inventory_row_maps_to_exactly_one_family` holds every row of #6970 to one family.
  "docs/policy-inventory-families.md",
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
  "app/src/ui/grip/grip.css",
  "app/src/ui/list/list.css",
  "app/src/ui/menu/menu.css",
  "app/src/ui/meter/meter.css",
  "app/src/ui/notice/notice.css",
  "app/src/ui/pane-foot/pane-foot.css",
  "app/src/ui/panel/panel.css",
  "app/src/ui/popover/popover.css",
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
  "e2e/fakeGitLab.test.ts",
  "e2e/fence.test.ts",
  "e2e/gitAnswer.test.ts",
  "e2e/recordedForge.test.ts",
  "e2e/standInForge.test.ts",
  "e2e/upgrade.test.ts",
  "src/LoopPauseAsk.window.test.tsx",
  "src/PersonaMark.test.tsx",
  "src/StripMarks.test.tsx",
  "src/allowLabels.test.ts",
  "src/artifactPage.test.ts",
  "src/askLabels.guard.test.ts",
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
  "src/ui/buildingDoc.test.ts",
  "src/ui/field/fieldHelp.guard.test.ts",
  "src/ui/field/refusingFields.guard.test.ts",
  "src/ui/menu/submenuRows.guard.test.ts",
  "src/ui/notice/noticeCardFill.test.ts",
  "src/waitingInItsTerminal.guard.test.ts",
  "src/windowControls.test.ts",
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
    "^(crates/studio-probe/|\\.github/actions/studio-probe-names/|crates/purlis-core/src/(executor|extension)(\\.rs|/)|crates/purlis-core/src/sandbox/(seatbelt|confined|egress|reach|wrapper|hosts|localport|door|tunnel|sshproxy|floor_tests|codex|opencode|claude)(\\.rs|/)|crates/purlis-core/src/names\\.rs|crates/purlis-core/src/hookwire/serve(\\.rs|/)|crates/purlis-core/src/secrets/(brokered|keyhold)(\\.rs|/)|crates/purlis-cli/src/listen/|crates/purlis-core/tests/it/a_sandboxed_chat_listens_nowhere_under_seatbelt\\.rs|crates/purlis-cli/tests/it/a_chats_ssh_route_carries_from_inside_its_sandbox\\.rs|Cargo\\.lock$|rust-toolchain\\.toml$)",
  "parity":
    "^(crates/(host|session-protocol|linux-sandbox|browser-engine|fake-harness|terminal|stand-in)/|crates/purlis-core/src/(sandbox|browser|hook|runner|eventlog|linkkey|host|dockerpool|dockercontext|pools|poolsetup|sshroute|tlsroute|codelink|session)[^/]*(\\.rs$|/)|crates/purlis-core/tests/it/a_remote_(docker|podman)_pool_(is_reached_(over_(ssh|tls)|through_a_docker_context)|doctor_names_each_fault)\\.rs$|crates/purlis-core/tests/it/an_ssh_runner_(takes_a_new_version_beside_the_old|host_outlives_the_sign_out_and_a_restart)\\.rs$|crates/purlis-core/tests/it/(a_live_remote_chat_survives_the_faults_that_define_trust|a_missing_real_infra_fixture_fails_in_ci|a_chat_moves_to_a_docker_pools_runner_and_back|a_docker_pool_runner_is_linked_through_docker_exec|a_killed_docker_exec_closes_what_it_ran_on_a_runner|code_crosses_to_a_docker_pools_runner_and_back)\\.rs$|crates/purlis-core/tests/support/(outage|real_infra|remote_rig)\\.rs$|crates/purlis-cli/src/(bridge|mcp|hooks|browsing|main)[^/]*(\\.rs$|/)|app/src-tauri/plugin/|app/src-tauri/src/(behaviour|host|session|hooks|killswitch|browser|chatbrowsers|routedsessions|runner|dockerpools|poolsetuptab|sandboxing|poolfollow)[^/]*(\\.rs$|/)|runner-image/|tests/(parity|remote-docker|remote-podman|ssh-runner|remote-faults)/|tools/fixture-images\\.mjs$|Cargo\\.lock$|rust-toolchain\\.toml$|\\.github/workflows/ci\\.yml$)",
  // No step of its own greps this one: the job's steps are the scenario legs'. What the Skill
  // studio's outcome bars measure, and the suite and its stand-ins (#4343).
  "outcome-bars":
    "^(app/src/(Skill|UseSkill|skill|chatSkill|personaSkills)|app/src/ui/list/|app/src-tauri/src/(skill|useskill|personaskills)|crates/skills/|crates/purlis-core/src/(skills|skillspan|skilltools|skilluse)(\\.rs|/)|crates/fake-harness/|app/e2e/(outcomeBars|harness\\.ts|wdio\\.outcome\\.conf|specs/skill-outcome-bars)|Cargo\\.toml$|\\.github/workflows/ci\\.yml$)",
  // No step of its own greps this one either: Pool setup's walks in the window on Docker (#4985)
  // and on a kind cluster (#4494), what they press and the pools and runner image they make, and,
  // for the cluster, the operator this build makes, what Install applies and the cluster's make.
  "pool-setup":
    "^(app/src/(poolsetup/|PoolSetup|AddDockerPool|pools\\.ts|settings/pools\\.tsx$)|app/src-tauri/src/(poolsetuptab|dockerpools|clusterpools|runnerlinks|routedsessions)[^/]*(\\.rs$|/)|crates/purlis-core/src/(poolsetup|dockerpool|clusterpool|runners|pools|sshroute|tlsroute)[^/]*(\\.rs$|/)|crates/cluster-doctor/|operator/(src/|deploy/|vendor/|Dockerfile$|Cargo\\.(toml|lock)$|e2e/kind-node\\.txt$)|runner-image/|tests/(remote-docker|remote-podman|kind-cluster)/|app/e2e/(pool-runner\\.Dockerfile$|wdio\\.pool\\.conf|poolsetup\\.ts$|specs/[^/]*\\.pool\\.e2e\\.ts$)|\\.github/(workflows/ci\\.yml|actions/kind/))",
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
  { crate: "purlis-core", test: "every_repo_path_comes_from_the_resolver" },
];

// The Rust tests that read every file of the repository: `ci.yml`'s `recorded` job runs each one
// when `rust` is out of scope, so no change skips them.
export const WHOLE_TREE_GUARDS = [
  { crate: "purlis-core", test: "every_first_party_source_file_carries_its_licence_header" },
  { crate: "purlis-core", test: "no_tracked_file_is_one_a_sandboxed_chat_may_not_write" },
  { crate: "purlis-core", test: "a_no_seed_mark_is_made_only_in_the_window" },
  { crate: "purlis-core", test: "only_attendance_compares_permission_mode_with_bypass" },
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
  if (listed(DOCS_READ_BY_RUST, path)) return RUST;
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
export function scopesOf({ event, paths, ciToolNames, deleted = new Set(), draft = false }) {
  if (event !== "pull_request") return { scopes: ALL, why: [] };
  if (draft) {
    const { scopes, why } = scopesOf({ event, paths, ciToolNames, deleted });
    return { scopes: new Set([...scopes].filter((s) => !HEAVY.includes(s))), why };
  }
  if (paths.length === 0) return { scopes: ALL, why: [] };
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
// A draft starts none of them: they are heavy jobs, started when it is marked ready (#7335).
export function gatesOf({ event, paths, draft = false }) {
  const open = event !== "pull_request" || paths.length === 0;
  return Object.fromEntries(
    Object.entries(GATES).map(([gate, pattern]) => {
      const re = new RegExp(pattern);
      if (event === "pull_request" && draft) return [gate, false];
      return [gate, open || paths.some((path) => re.test(path))];
    }),
  );
}

// Which parts of `web` a change needs: part 2 holds the tools' tests, which run on every change.
// The other three would each take a runner to say there is nothing to do.
export const webParts = (scopes) => (scopes.has("web") ? [1, 2, 3, 4] : [2]);

// The label that asks a pull request's `stories` run to draw the baselines it would need
// (`stories-baselines.yml` commits them).
export const BASELINES_LABEL = "update-baselines";

// The browsers `stories.yml` runs: WebKit alone on a ready pull request, which is where the two
// diverge most, and both on the queue, main and a nightly run (#7335). Chromium's pictures are
// held there, and the queue draws none. So a pull request that asks for baselines (the
// `update-baselines` label, `labels` as the names joined by commas) runs Chromium as well: its
// run is the only one that can draw a Chromium picture, and a Chromium-only picture change would
// otherwise first fail in the queue with no way to take the baseline.
export const storiesBrowsers = ({ event, labels = "" }) =>
  event !== "pull_request" || labels.split(",").includes(BASELINES_LABEL)
    ? ["chromium", "webkit"]
    : ["webkit"];

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

// Paths a dev build never reads. A pattern only matches a path it is certain of; the rest ships.
const NOT_SHIPPED = [
  /^(docs|research)\//,
  // A test: Rust's integration tests, vitest and node's, the scenario specs, the stories and the
  // story baselines. A Rust unit test lives in the source file it tests, so it ships.
  /^crates\/[^/]+\/tests\//,
  /^app\/e2e\//,
  /^app\/src\/.*\.(test|stories)\.(tsx?|mjs)$/,
  /(^|\/)__screenshots__\//,
  /^tools\/[^/]+\.test\.mjs$/,
  // A workflow other than the release one and the one it calls (`runner-image.yml`): `ci`,
  // `docs`, `stories` and the rest build nothing that is published.
  /^\.github\/workflows\/(?!(release|runner-image)\.yml$)[^/]+\.ya?ml$/,
];

/** Whether a dev build reads this path. */
export function pathShips(path) {
  if (path === "CHANGELOG.md") return true;
  if (NOT_SHIPPED.some((re) => re.test(path))) return false;
  // Markdown under `crates/` and `app/` is compiled or bundled into a build: `purlis-core`'s
  // `docs/` and `templates/` (`build.rs`) and the plugin's skills (`app/src-tauri/plugin/`). Every
  // other Markdown file is a page for people.
  if (path.endsWith(".md")) return /^(crates|app)\//.test(path);
  return true;
}

// Paths only a Linux build reads (#7324): the runner image and its workflow, the `.deb`'s
// maintainer scripts, and the sandbox crate's Linux code. `purlis-core`, the CLI and the app all
// depend on `purlis-linux-sandbox` unconditionally, so its cross-platform modules (`lib.rs`,
// `profile`, `carried`, `refusal`, its `Cargo.toml`) are in the macOS build's graph and are not
// listed: only `src/linux/` and `loopback`, which are `cfg(target_os = "linux")` (`loopback` also
// `cfg(test)`), compile to nothing there. A pattern only matches a path it is certain of.
const LINUX_ONLY = [
  /^runner-image\//,
  /^\.github\/workflows\/runner-image\.yml$/,
  /^app\/src-tauri\/packaging\/deb\//,
  /^crates\/linux-sandbox\/src\/linux\//,
  /^crates\/linux-sandbox\/src\/loopback(\/|\.rs$)/,
];

/**
 * Whether partial dev builds are ON: a dev build of a change that only Linux reads skips macOS.
 *
 * **OFF, and it stays off until a partial publish is safe.** `dev.json` names one version for
 * every platform and each platform's asset is signed over its own bytes. A Linux-only build
 * would announce version N+1 beside the previous macOS archive, built as N: the updater
 * installs it, the app reports N, and offers N+1 again for ever; and the macOS runner binary
 * the desktop looks for by version (`runnerinstall::asset_url`) would be missing. `build`,
 * `sbom`, `provenance`, `runner-binaries` and `symbols` also need both legs. The decision is
 * here and tested, but `plan` reads only `ships=false` and nothing yet reads `macos` or `linux`: turning
 * it on is this constant plus the workflow work above, and a test that no macOS-built crate reads a
 * `LINUX_ONLY` path.
 */
export const PARTIAL_DEV_BUILDS = false;

/** Which platforms read at least one of `paths`: a path that ships and is not Linux-only is both. */
export function platformsOf(paths) {
  const shipped = paths.filter(pathShips);
  return {
    macos: shipped.some((path) => !LINUX_ONLY.some((re) => re.test(path))),
    linux: shipped.length > 0,
  };
}

/**
 * Whether a build of `kind` has anything to build, and why, and for which platforms. Only a
 * `dev` build (a push to `main`) can be skipped; `paths` is every path changed since the
 * published commit, or `null` when that could not be read. `macos` and `linux` say which
 * platforms build: always both when anything ships, unless `partial` (`PARTIAL_DEV_BUILDS`).
 */
export function shipsOf({ kind, paths, partial = PARTIAL_DEV_BUILDS }) {
  const both = { macos: true, linux: true };
  if (kind !== "dev") return { ships: true, ...both, why: `a ${kind} build always builds` };
  if (paths === null || paths.length === 0) {
    return { ships: true, ...both, why: "no change since the published commit could be read" };
  }
  const shipped = paths.filter(pathShips);
  if (shipped.length === 0) {
    return {
      ships: false,
      macos: false,
      linux: false,
      why: `nothing a build reads changed (${paths.slice(0, 5).join(", ")}${paths.length > 5 ? ", ..." : ""})`,
    };
  }
  const named = `it ships ${shipped.slice(0, 5).join(", ")}`;
  const { macos, linux } = platformsOf(paths);
  if (macos || !partial) return { ships: true, ...both, why: named };
  return { ships: true, macos, linux, why: `${named}, all of it Linux-only: no macOS build` };
}

const COMPARE_PAGE = 100;
const COMPARE_PAGES = 3;

/**
 * [`shipsOf`] for the push of `sha`, read from the forge through `api` (a function from `gh api`'s
 * arguments to what it printed, throwing when it fails): the paths changed between the commit the
 * published `dev.json` names and `sha`. Whatever cannot be read says `ships`.
 */
export function askShips({ repo, kind, sha, api, partial }) {
  if (kind !== "dev") return shipsOf({ kind, paths: null, partial });
  try {
    const published = readPublished({ repo, api });
    if (published === null) return { ships: true, macos: true, linux: true, why: "the dev channel serves nothing yet" };
    const paths = [];
    for (let page = 1; page <= COMPARE_PAGES; page += 1) {
      const compared = JSON.parse(
        api([`repos/${repo}/compare/${published.sha}...${sha}?per_page=${COMPARE_PAGE}&page=${page}`]),
      );
      // Only a build ahead of the published one is measured; `dev-build-order.mjs` rules on any
      // other standing (behind, identical, diverged).
      if (compared.status !== "ahead") {
        return { ships: true, macos: true, linux: true, why: `the compare says ${compared.status}, which is not a change to measure` };
      }
      const files = compared.files ?? [];
      for (const file of files) {
        paths.push(file.filename);
        if (file.previous_filename) paths.push(file.previous_filename);
      }
      if (files.length < COMPARE_PAGE) {
        const answer = shipsOf({ kind, paths, partial });
        return { ...answer, why: `since ${published.sha}: ${answer.why}` };
      }
    }
    return { ships: true, macos: true, linux: true, why: "the change is too long for the forge to list in full" };
  } catch (error) {
    return { ships: true, macos: true, linux: true, why: `the change could not be read (${error.message})` };
  }
}

function ghApi(args) {
  return execFileSync("gh", ["api", ...args], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    maxBuffer: 1 << 28,
  });
}

// `ships <kind> <sha>`: writes `ships=`, `macos=` and `linux=` (`true|false`) to `$GITHUB_OUTPUT` (or stdout) and says why.
function mainShips([kind, sha]) {
  const out = process.env.GITHUB_OUTPUT;
  const say = (line) => (out ? appendFileSync(out, `${line}\n`) : console.log(line));
  const repo = process.env.GITHUB_REPOSITORY;
  const answer = repo
    ? askShips({ repo, kind, sha, api: ghApi })
    : { ships: true, macos: true, linux: true, why: "GITHUB_REPOSITORY is not set" };
  say(`ships=${answer.ships}`);
  say(`macos=${answer.macos}`);
  say(`linux=${answer.linux}`);
  console.log(`ships=${answer.ships}: ${answer.why}`);
}

/**
 * The path filter of a workflow's `pull_request` trigger, in order, or `null` when it has none
 * (#7420). Read line by line as the other tools read workflows: the `paths:` list under
 * `pull_request:` in the `on:` block, a comment and the quotes around a pattern dropped.
 */
export function pullRequestPathsOf(text) {
  const lines = text.split("\n");
  const on = lines.findIndex((line) => /^on:\s*$/.test(line));
  if (on < 0) return null;
  let at = -1;
  for (let i = on + 1; i < lines.length; i++) {
    if (/^\S/.test(lines[i]) && !/^#/.test(lines[i])) break;
    if (/^ {2}pull_request:/.test(lines[i])) {
      at = i;
      break;
    }
  }
  if (at < 0) return null;
  const patterns = [];
  let inPaths = false;
  for (const line of lines.slice(at + 1)) {
    if (/^ {0,2}\S/.test(line) && !/^ {0,2}#/.test(line)) break;
    if (/^ {4}paths:\s*$/.test(line)) {
      inPaths = true;
      continue;
    }
    if (/^ {4}\S/.test(line)) inPaths = false;
    const item = inPaths ? /^ {6}- +(['"]?)(.+?)\1\s*(#.*)?$/.exec(line) : null;
    if (item) patterns.push(item[2]);
  }
  return patterns.length > 0 ? patterns : null;
}

/**
 * Whether a change to `paths` starts a workflow whose `pull_request` filter is `patterns`, as
 * GitHub reads it: a path is in when the last line it matches is not a `!` one. No filter, and a
 * change with no paths (one that could not be read), start it: a fault runs more.
 */
export function workflowNeeded(patterns, paths) {
  if (patterns === null || paths.length === 0) return true;
  // GitHub's `*` and `**` match a name that starts with a dot; `matchesGlob`'s do not. A leading
  // dot of any segment is swapped for a letter no path holds, on both sides, so they do.
  const plain = (text) => text.replace(/(^|\/)\./g, "$1\u0001");
  return paths.some((path) => {
    let hit = false;
    for (const pattern of patterns) {
      const negated = pattern.startsWith("!");
      if (matchesGlob(plain(path), plain(negated ? pattern.slice(1) : pattern))) hit = !negated;
    }
    return hit;
  });
}

// `workflow <file> <base>`: writes `run=true|false` to `$GITHUB_OUTPUT` (or stdout), for a merge
// group's run of a workflow that a pull request starts by `paths:` (#7420). A merge group's event
// has no `paths`, so the queue run reads the workflow's own filter against what the change
// touches since `base` (`change-base`): the two cannot drift. Any failure to read says `true`.
function workflowRuns([file, base]) {
  const out = process.env.GITHUB_OUTPUT;
  const say = (line) => (out ? appendFileSync(out, `${line}\n`) : console.log(line));
  let run = true;
  let why = "";
  try {
    const patterns = pullRequestPathsOf(readFileSync(file, "utf8"));
    const paths = execFileSync("git", ["diff", "--name-only", "--no-renames", base, "HEAD"], {
      encoding: "utf8",
      maxBuffer: 1 << 28,
    })
      .split("\n")
      .filter(Boolean);
    run = workflowNeeded(patterns, paths);
    const hits = patterns === null ? [] : paths.filter((p) => workflowNeeded(patterns, [p]));
    why = run ? `: ${hits.slice(0, 5).join(", ") || "no filter"}` : ` (nothing of ${paths.length} changed paths is in its filter)`;
  } catch (error) {
    console.log(`::warning::the change could not be read, so the workflow runs: ${error.message}`);
  }
  say(`run=${run}`);
  console.log(`::notice title=Scope::${file} ${run ? "runs" : "is skipped"}${why}`);
}

function main([event, base, draftArg, labels = ""]) {
  const draft = draftArg === "true";
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
    result = scopesOf({ event, paths, ciToolNames, deleted, draft });
    gates = gatesOf({ event, paths, draft });
    console.log(paths.join("\n"));
  } catch (error) {
    console.log(`::warning::the change could not be read, so every job runs: ${error.message}`);
    result = { scopes: ALL, why: [] };
    gates = gatesOf({ event, paths: [] });
  }
  for (const scope of SCOPES) say(`${scope}=${result.scopes.has(scope)}`);
  for (const [gate, on] of Object.entries(gates)) say(`${gate}=${on}`);
  say(`web-parts=${JSON.stringify(webParts(result.scopes))}`);
  say(`stories-browsers=${JSON.stringify(storiesBrowsers({ event, labels }))}`);
  const skipped = SCOPES.filter((s) => !result.scopes.has(s));
  if (skipped.length === 0) {
    const because = result.why.length > 0 ? `: ${result.why.slice(0, 5).join(", ")}` : "";
    console.log(`::notice title=Scope::every job runs (${event}${because})`);
  } else {
    const runs = SCOPES.filter((s) => result.scopes.has(s));
    console.log(`::notice title=Scope::runs ${runs.join(", ")}; skips ${skipped.join(", ")}`);
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [first, ...rest] = process.argv.slice(2);
  if (first === "ships") mainShips(rest);
  else if (first === "workflow") workflowRuns(rest);
  else main([first, ...rest]);
}
