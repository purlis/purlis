// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// No release build carries a crate's test-only feature (#2216, #3180).
//
// `TEST_ONLY` lists them: `purlis-session-protocol`'s `any-stream` (its test-only stand-ins: a
// link served over any stream, a device taken as proved, the link's stand-in handshake),
// `purlis-core`'s `fenced` (the plane fence), and every crate's `test-support` (what a test of
// another crate reaches in, such as `purlis-skills`' mover hook). Their tests ask for them from
// `[dev-dependencies]`, which Cargo leaves out of a build that is not a test. This reads every
// build the shipping workflows make, asks Cargo which features that build resolves for each
// crate (`cargo tree -e normal,build,features`, every target, nothing compiled), and fails with
// an error on the manifest that turned a feature on.
//
// The list is checked against the workspace too: a package's `test-support` feature that
// `TEST_ONLY` leaves out, a listed feature no package has, and one with no `CANARY` are errors,
// so a new test hook is caught here rather than kept out by the resolver alone.
//
// It fails closed. A build line it cannot read whole, a feature flag on one, a cargo
// subcommand it does not know, cargo called by path or through a variable, a workflow that
// builds and is in neither of its lists, a workflow listed as not shipping that publishes, a
// shipping workflow with no build it can read,
// `cargo tree` output it cannot parse, or a canary that no longer finds the feature where a
// dev-dependency does turn it on: each is an error, so a change in how a binary is built or in
// what Cargo prints cannot quietly turn the check off.
//
// Not cargo-deny's `bans.features`: the `deny` job runs with `--all-features` and cargo-deny
// resolves dev-dependencies into its graph, so the feature the tests ask for is always there;
// it cannot say "in no build that ships".
//
// Run by the `lint` job from the repository root: `node tools/release-features.mjs`.

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join, relative } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

/** The workflows that build a binary someone is given. */
export const SHIPPING_WORKFLOWS = ["release.yml", "runner-image.yml", "operator-image.yml"];

/**
 * The workflows that build with cargo or tauri and give nobody what they build, each with why.
 * A workflow that builds is in this list or in `SHIPPING_WORKFLOWS`, and one in this list
 * publishes nothing; anything else is an error.
 */
export const NOT_SHIPPING = {
  "ci.yml": "checks every pull request and main: its builds are tested, linted and measured, never handed on",
  "forge-live.yml": "runs the forge contract tests against a real forge: it compiles tests only",
  "mutants.yml": "mutation testing of the core crate: its builds are only ever tested",
  "operator.yml": "checks the cluster operator and runs it on a throwaway kind cluster",
  "stress.yml": "the stress and scale measurements: its builds run tests and benchmarks only",
};

/** The name a crate gives the feature only another crate's tests turn on; each is in `TEST_ONLY`. */
export const TEST_SUPPORT = "test-support";

/** The features only a test may turn on. */
export const TEST_ONLY = [
  { crate: "purlis-session-protocol", feature: "any-stream" },
  { crate: "purlis-core", feature: "fenced" },
  { crate: "purlis-core", feature: TEST_SUPPORT },
  { crate: "purlis-host", feature: TEST_SUPPORT },
  { crate: "purlis-skills", feature: TEST_SUPPORT },
  { crate: "purlis-terminal", feature: TEST_SUPPORT },
];

/**
 * A crate whose `[dev-dependencies]` turns each test-only feature on: the canary's build. Never
 * the feature's own crate, whose tests ask for it through a dependency on itself.
 */
export const CANARY = {
  "purlis-session-protocol/any-stream": "purlis-host",
  "purlis-core/fenced": "purlis-cli",
  "purlis-core/test-support": "purlis-cli",
  "purlis-host/test-support": "purlis-app",
  "purlis-skills/test-support": "purlis-core",
  "purlis-terminal/test-support": "purlis-core",
};

/** The Tauri project `tauri build` builds, run from `app/`. */
export const TAURI_PROJECT = "app/src-tauri";

// ---------------------------------------------------------------------------------------------
// The list, against the workspace

/**
 * The errors in `testOnly` and `canary` against the workspace's `packages` (`cargo metadata`'s,
 * each `{ name, features }`): a `test-support` feature the list leaves out, a listed feature no
 * package has, and one with no canary.
 */
export function listErrors(packages, testOnly = TEST_ONLY, canary = CANARY) {
  const listed = new Set(testOnly.map(({ crate, feature }) => `${crate}/${feature}`));
  const has = new Set(packages.flatMap(({ name, features }) => Object.keys(features ?? {}).map((f) => `${name}/${f}`)));
  const unlisted = packages
    .filter(({ name, features }) => Object.hasOwn(features ?? {}, TEST_SUPPORT) && !listed.has(`${name}/${TEST_SUPPORT}`))
    .map(({ name }) => `${name}/${TEST_SUPPORT} is a test-only feature TEST_ONLY does not list: list it, with a CANARY`);
  const gone = [...listed]
    .filter((f) => !has.has(f))
    .map((f) => `TEST_ONLY lists ${f}, and no package of the workspace has it`);
  const unwatched = [...listed]
    .filter((f) => has.has(f) && !Object.hasOwn(canary, f))
    .map((f) => `TEST_ONLY lists ${f} with no CANARY to show this check still sees it`);
  return [...unlisted, ...gone, ...unwatched];
}

// ---------------------------------------------------------------------------------------------
// `cargo tree --prefix depth -i <crate> -e …,features`

/** Each line of `cargo tree --prefix depth` as `{ depth, text }`; a line of another shape throws. */
export function treeNodes(tree) {
  return tree
    .split("\n")
    .filter((line) => line.trim() !== "")
    .map((line) => {
      const match = /^(\d+)(\S.*)$/.exec(line);
      if (!match) throw new Error(`cargo tree printed a line this check cannot read: ${JSON.stringify(line)}`);
      return { depth: Number(match[1]), text: match[2] };
    });
}

/** `pkg feature "f"` as `{ name, feature }`, or a package node `pkg vX (…)` as `{ name, feature: null }`. */
function node(text) {
  const asFeature = /^(\S+) feature "([^"]+)"/.exec(text);
  if (asFeature) return { name: asFeature[1], feature: asFeature[2] };
  return { name: text.split(" ")[0], feature: null };
}

function spelled({ name, feature }) {
  return feature === null ? name : `${name} feature \`${feature}\``;
}

/**
 * The crates that turn `feature` on, from an inverted features tree of `crate`: each node right
 * under the feature's own. A package node is a dependency line in that crate's manifest
 * (`via: null`); a feature node is one of that crate's features (`via: <name>`). `askedBy` is
 * the first chain further up, from the node above the enabler to the build's own package.
 *
 * Throws unless the tree is rooted at `crate` and shows at least one feature node, so output
 * whose shape has changed is an error rather than "nothing found".
 */
export function enablers(tree, { crate, feature }) {
  const nodes = treeNodes(tree);
  if (nodes.length === 0 || nodes[0].depth !== 0 || !nodes[0].text.startsWith(`${crate} v`)) {
    throw new Error(`cargo tree's output is not rooted at ${crate}: ${JSON.stringify(nodes[0]?.text ?? "")}`);
  }
  if (!nodes.some((n) => / feature "[^"]+"/.test(n.text))) {
    throw new Error(`cargo tree's output for ${crate} shows no feature node: was it run with -e features?`);
  }
  const own = `${crate} feature "${feature}"`;
  const found = [];
  let featureDepth = null;
  for (const [i, n] of nodes.entries()) {
    if (featureDepth !== null && n.depth <= featureDepth) featureDepth = null;
    if (featureDepth === null) {
      if (n.text.trim() === own) featureDepth = n.depth;
      continue;
    }
    if (n.depth !== featureDepth + 1) continue;
    const { name, feature: via } = node(n.text);
    if (found.some((f) => f.name === name && f.via === via)) continue;
    // Further up: the first path from this node to the build's own package. A package's own
    // node and the `default` feature its dependents ask for say nothing new, so they are left out.
    const askedBy = [];
    let previous = { name, feature: via };
    for (let j = i + 1, depth = n.depth; j < nodes.length && nodes[j].depth === depth + 1; j++, depth++) {
      const up = node(nodes[j].text);
      const samePackage = up.name === previous.name && previous.feature === null;
      if (!(up.feature === null && up.name === previous.name) && !(samePackage && up.feature === "default")) {
        askedBy.push(spelled(up));
      }
      previous = up;
    }
    found.push({ name, via, askedBy });
  }
  return found;
}

/** An `::error` line on each enabling crate's manifest. */
export function annotations({ found, test, build, manifests, root }) {
  return found.map(({ name, via, askedBy }) => {
    const manifest = manifests.get(name);
    const file = manifest ? relative(root, manifest) : "Cargo.toml";
    const where = via === null ? "in its [dependencies]" : `from its feature \`${via}\``;
    const chain = askedBy.length > 0 ? ` (asked for by ${askedBy.join(" <- ")})` : "";
    return (
      `::error file=${file},title=test-only feature in a release build::` +
      `${name} turns on ${test.crate}'s \`${test.feature}\` ${where}${chain}, ` +
      `so ${build} carries it. Ask for it from [dev-dependencies] only.`
    );
  });
}

// ---------------------------------------------------------------------------------------------
// The builds a workflow makes

/** Cargo's flags a release build line may carry, and whether each takes a value. */
const CARGO_FLAGS = new Map([
  ["--profile", true], ["--target", true], ["--bin", true], ["--target-dir", true],
  ["-j", true], ["--jobs", true], ["--manifest-path", true], ["-p", true], ["--package", true],
  ["--release", false], ["-r", false], ["--locked", false], ["--frozen", false], ["--offline", false],
  ["--bins", false], ["-v", false], ["--verbose", false], ["-q", false], ["--quiet", false],
]);
/** The cargo subcommands that build. */
const BUILDS = ["build", "b", "rustc"];
/** The cargo subcommands that run what they compile: a build when given a release profile. */
const RUNS = ["run", "r"];
const TESTS = ["test", "t"];
/** The flags that make `cargo run` or `cargo test` compile as a release build would. */
const RELEASE_FLAGS = ["--release", "-r", "--profile", "--no-run"];
/**
 * The cargo subcommands that make nothing someone is given: a test, a lint, a report, a tool
 * the workflow runs for itself (`run` and `test` only without `RELEASE_FLAGS`). Any other
 * subcommand is an error until it is read here. `--version` and `-V` are flags, there for
 * `cargo --version`.
 */
const NOT_BUILDS = [
  ...RUNS, ...TESTS, "check", "c", "clippy", "fmt", "tree", "metadata", "deny", "audit",
  "fetch", "update", "locate-project", "pkgid", "generate-lockfile", "help", "version", "--version", "-V",
];
const CARGO_FEATURE_FLAGS = ["--features", "-F", "--all-features", "--no-default-features"];

/** Tauri's flags a release build line may carry before `--`, and whether each takes a value. */
const TAURI_FLAGS = new Map([
  ["--config", true], ["-c", true], ["--bundles", true], ["-b", true], ["--runner", true],
  ["-r", true], ["--target", true], ["-t", true],
  ["--debug", false], ["-d", false], ["--no-bundle", false], ["--verbose", false],
  ["-v", false], ["--ci", false],
]);
const TAURI_FEATURE_FLAGS = ["--features", "-f"];

/** `--flag=value` and `-pvalue` as `[flag, value]`; anything else as `[token, undefined]`. */
function splitFlag(token, flags) {
  const eq = token.indexOf("=");
  if (token.startsWith("--") && eq !== -1) return [token.slice(0, eq), token.slice(eq + 1)];
  if (/^-[A-Za-z]./.test(token) && flags.get(token.slice(0, 2))) return [token.slice(0, 2), token.slice(2)];
  if (/^-F./.test(token) || /^-f./.test(token)) return [token.slice(0, 2), token.slice(2)];
  return [token, undefined];
}

function unquote(token) {
  return token.replace(/^"(.*)"$/s, "$1").replace(/^'(.*)'$/s, "$1");
}

/** Reads Cargo's arguments after `build` into `into`; returns an error or null. */
function cargoArgs(tokens, into) {
  for (let i = 0; i < tokens.length; i++) {
    const [flag, inline] = splitFlag(tokens[i], CARGO_FLAGS);
    if (CARGO_FEATURE_FLAGS.includes(flag)) return `a feature flag (${tokens[i]}) on a release build`;
    if (!CARGO_FLAGS.has(flag)) return `\`${tokens[i]}\` is not a flag this check reads`;
    if (!CARGO_FLAGS.get(flag)) continue;
    const value = inline ?? tokens[++i];
    if (value === undefined || value === "") return `\`${flag}\` has no value`;
    if (flag === "-p" || flag === "--package") into.packages.push(unquote(value));
    if (flag === "--manifest-path") into.manifest = unquote(value);
  }
  return null;
}

/** Reads `tauri build`'s arguments, and Cargo's after `--`, into `into`; returns an error or null. */
function tauriArgs(tokens, into) {
  const split = tokens.indexOf("--");
  const own = split === -1 ? tokens : tokens.slice(0, split);
  for (let i = 0; i < own.length; i++) {
    const [flag, inline] = splitFlag(own[i], TAURI_FLAGS);
    if (TAURI_FEATURE_FLAGS.includes(flag)) return `a feature flag (${own[i]}) on a release build`;
    if (!TAURI_FLAGS.has(flag)) return `\`${own[i]}\` is not a flag this check reads`;
    if (TAURI_FLAGS.get(flag) && inline === undefined) i++;
  }
  if (split === -1) return null;
  const cargo = { manifest: null, packages: [] };
  const error = cargoArgs(tokens.slice(split + 1), cargo);
  if (error) return error;
  if (cargo.manifest !== null) return "`--manifest-path` after `tauri build --`";
  into.packages.push(...cargo.packages);
  return null;
}

/** One build line's words, shell assignments in front dropped (not one that runs a command). */
function words(command) {
  const tokens = command.replace(/\$\{\{[^}]*\}\}/g, "EXPR").trim().split(/\s+/);
  while (tokens.length > 0 && /^[A-Za-z_][A-Za-z0-9_]*=(?!\$?\()/.test(tokens[0])) tokens.shift();
  return tokens;
}

/** A shell expansion standing for a command: `$CARGO`, `${CARGO}`, `${CARGO:-cargo}`. */
const VARIABLE = /^\$(\{[^}]*\}|[A-Za-z_][A-Za-z0-9_]*)$/;
/** The words after which a word is a command. */
const SEPARATORS = ["&&", "||", ";", "|", "!", "then", "do", "else", "-c", "exec", "time", "env", "sudo", "xargs"];

/** Cargo's flags up to `--`: what the build reads; after it are the program's own arguments. */
function ownArgs(tokens) {
  const split = tokens.indexOf("--");
  return split === -1 ? tokens : tokens.slice(0, split);
}

/** Whether `cargo run` or `cargo test` at `at` compiles as a release build would. */
function releaseRun(tokens, at) {
  return ownArgs(tokens.slice(at + 1)).some((t) => RELEASE_FLAGS.includes(t.split("=")[0]));
}

/**
 * Each word in `tokens` that calls cargo, and the subcommand after it (past a `+toolchain` and
 * `auditable`): `{ k, at, sub, how }`. `how` is `name` for `cargo` itself, `path` for one by
 * path or `cargo.exe`, `plugin` for a `cargo-<name>` binary in a command's place, and
 * `variable` for an expansion in a command's place whose name says cargo, or anywhere before a
 * build's words.
 */
function cargoCalls(tokens) {
  return tokens.flatMap((token, k) => {
    const opened = token.replace(/^[A-Za-z_][A-Za-z0-9_]*=/, "").replace(/^\$?\(+/, "");
    const word = opened.replace(/^["']/, "").replace(/["');]+$/, "");
    const command = k === 0 || opened !== token || SEPARATORS.includes(tokens[k - 1]) || /[;&|]$/.test(tokens[k - 1]);
    let at = k + 1;
    if (tokens[at]?.startsWith("+")) at++;
    if (tokens[at] === "auditable") at++;
    const sub = tokens[at];
    if (word === "cargo") return [{ k, at, sub, how: "name" }];
    if (/(^|\/)cargo\.exe$|\/cargo$/.test(word)) return [{ k, at, sub, how: "path" }];
    if (command && /^cargo-[a-z]/.test(word)) return [{ k, at, sub, how: "plugin" }];
    if (VARIABLE.test(word) && ((command && /cargo/i.test(word)) || at > k + 1 || BUILDS.includes(sub))) {
      return [{ k, at, sub, how: "variable" }];
    }
    return [];
  });
}

/** Why a cargo call is one this check cannot read, or null when it is a build or not one. */
function unreadCall({ at, sub, how }, tokens) {
  if (how === "variable") return "cargo called through a variable is a build this check cannot read: call `cargo` by name";
  if (how === "plugin") return "a cargo plugin's own binary is a build this check cannot read: call it as `cargo <subcommand>`";
  if (how === "path") {
    return NOT_BUILDS.includes(sub) ? null : "cargo called by path is a build this check cannot read: call `cargo` by name";
  }
  if (TESTS.includes(sub) && releaseRun(tokens, at)) {
    return "a test build with a release profile, or one that only compiles, is a build this check does not read";
  }
  if (sub === undefined || BUILDS.includes(sub) || NOT_BUILDS.includes(sub) || sub === "install") return null;
  if (sub.startsWith("-")) return `\`cargo ${sub}\` before the subcommand is not a form this check reads`;
  // Prose after the word (an `echo` that names cargo) is not a subcommand.
  if (!/^[a-z][a-z0-9-]*$/.test(sub)) return null;
  return `\`cargo ${sub}\` is not a subcommand this check knows: if it compiles what ships, read its build here`;
}

/**
 * Each build a workflow's text makes: `{ line, manifest, packages }`, where `packages` empty
 * means the manifest's own package, or `{ line, error }` for one this check cannot read whole.
 * `cargo install` of a published crate (a tool) is not a build of this repository's.
 */
export function builds(workflow) {
  const lines = workflow.split("\n");
  const found = [];
  for (let i = 0; i < lines.length; i++) {
    const line = i + 1;
    let command = lines[i];
    while (command.trimEnd().endsWith("\\") && i + 1 < lines.length) {
      command = command.trimEnd().slice(0, -1) + " " + lines[++i];
    }
    const text = command.replace(/^\s*(-\s+)?(run:\s*)?/, "");
    if (text.trimStart().startsWith("#")) continue;
    const tauri = /\btauri\s+build\b/.test(text);
    const tokens = words(text);
    if (tauri) {
      const at = tokens.indexOf("build");
      const head = tokens.slice(0, at).join(" ");
      if (head !== "npx tauri" && head !== "tauri") {
        found.push({ line, error: `\`${head} build\` is not a tauri build this check reads` });
        continue;
      }
      const into = { manifest: `${TAURI_PROJECT}/Cargo.toml`, packages: [] };
      const error = tauriArgs(tokens.slice(at + 1), into);
      found.push(error ? { line, error } : { line, ...into });
      continue;
    }
    const calls = cargoCalls(tokens);
    const unread = calls.map((call) => unreadCall(call, tokens)).find((error) => error !== null);
    if (unread) {
      found.push({ line, error: unread });
      continue;
    }
    const building = calls.filter(
      ({ sub, at }) =>
        BUILDS.includes(sub) ||
        (RUNS.includes(sub) && releaseRun(tokens, at)) ||
        (sub === "install" && tokens.slice(at).some((t) => /^--(path|git)(=|$)/.test(t))),
    );
    if (building.length === 0) continue;
    const [{ k, at, sub }] = building;
    if (k !== 0 || tokens[k] !== "cargo" || building.length > 1) {
      found.push({ line, error: "a cargo build after other words is not one this check reads" });
      continue;
    }
    if (sub === "install") {
      found.push({ line, error: "`cargo install` from a path or a git repository is a build this check does not read" });
      continue;
    }
    const into = { manifest: "Cargo.toml", packages: [] };
    // `cargo run`'s arguments after `--` are its program's, not the build's.
    const own = RUNS.includes(sub) ? ownArgs(tokens.slice(at + 1)) : tokens.slice(at + 1);
    const error = cargoArgs(own, into);
    if (error) found.push({ line, error });
    else if (into.manifest === "Cargo.toml" && into.packages.length === 0) {
      found.push({ line, error: "a build of the root workspace with no `-p` builds its default members: name the packages" });
    } else found.push({ line, ...into });
  }
  return found;
}

/** What publishes a release or pushes an image, in a workflow's text: checked on `NOT_SHIPPING`. */
const PUBLISHES = [
  /\bgh\s+release\s+(create|upload|edit)\b/,
  /\buses:\s*["']?(softprops\/action-gh-release|ncipollo\/release-action|tauri-apps\/tauri-action|docker\/build-push-action)@/,
  /\b(docker|podman|buildah)\s+(image\s+)?push\b/,
  /\bbuildx\s+build\b.*\s--push\b/,
  /^\s*packages:\s*write\b/m,
];

/** Whether a workflow's text, its comments left out, publishes a release or pushes an image. */
function publishes(text) {
  const live = text.split("\n").filter((l) => !l.trimStart().startsWith("#")).join("\n");
  return PUBLISHES.some((publish) => publish.test(live));
}

/**
 * The errors on the workflows of `workflows` (`{ name, text }`), as `{ name, error }`: one that
 * builds with cargo or tauri (a build `builds()` reads, one it cannot, or `tauri-action`) and is
 * in neither `SHIPPING_WORKFLOWS` nor `NOT_SHIPPING`, and one in `NOT_SHIPPING` that publishes.
 */
export function workflowErrors(workflows) {
  return workflows.flatMap(({ name, text }) => {
    if (SHIPPING_WORKFLOWS.includes(name)) return [];
    if (Object.hasOwn(NOT_SHIPPING, name)) {
      return publishes(text)
        ? [{ name, error: `${name} is in NOT_SHIPPING and publishes: list it in SHIPPING_WORKFLOWS` }]
        : [];
    }
    const building = builds(text).length > 0 || /^\s*-?\s*uses:\s*["']?tauri-apps\/tauri-action@/m.test(text);
    return building
      ? [{ name, error: `${name} builds with cargo or tauri and is in neither SHIPPING_WORKFLOWS nor NOT_SHIPPING: list it in one, with why` }]
      : [];
  });
}

/** `build.features` in a Tauri config, which `tauri build` passes to Cargo. */
export function tauriConfigFeatures(config) {
  const features = config?.build?.features;
  return Array.isArray(features) ? features : features === undefined ? [] : [String(features)];
}

// ---------------------------------------------------------------------------------------------

function cargo(args) {
  return execFileSync("cargo", args, { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
}

function tree({ manifest, packages, crate, edges }) {
  return cargo([
    "tree", "--locked", "--manifest-path", manifest, "--prefix", "depth", "--target", "all",
    "-e", edges, "-i", crate, ...packages.flatMap((p) => ["-p", p]),
  ]);
}

function main() {
  const errors = [];
  const metadataOf = new Map();
  const metadata = (manifest) => {
    if (!metadataOf.has(manifest)) {
      metadataOf.set(manifest, JSON.parse(cargo(["metadata", "--locked", "--no-deps", "--format-version", "1", "--manifest-path", manifest])));
    }
    return metadataOf.get(manifest);
  };

  const listed = listErrors(metadata("Cargo.toml").packages);
  for (const error of listed) errors.push(`::error file=tools/release-features.mjs,title=the test-only list::${error}`);

  for (const test of TEST_ONLY) {
    const canary = CANARY[`${test.crate}/${test.feature}`];
    if (!canary) continue;
    const seen = enablers(tree({ manifest: "Cargo.toml", packages: [canary], crate: test.crate, edges: "normal,build,dev,features" }), test);
    if (!seen.some((f) => f.name === canary)) {
      errors.push(`::error file=tools/release-features.mjs,title=canary::${canary}'s dev-dependency turns on ${test.crate}'s \`${test.feature}\`, and this check no longer sees it there`);
    } else console.log(`canary: ${canary}'s tests turn on ${test.crate}/${test.feature}, as seen`);
  }

  for (const file of readdirSync(TAURI_PROJECT).filter((f) => /^tauri.*\.json$/.test(f))) {
    const features = tauriConfigFeatures(JSON.parse(readFileSync(join(TAURI_PROJECT, file), "utf8")));
    if (features.length > 0) {
      errors.push(`::error file=${TAURI_PROJECT}/${file},title=features in a Tauri config::build.features (${features.join(", ")}) goes to every tauri build that reads this file`);
    }
  }

  const workflows = readdirSync(".github/workflows")
    .filter((f) => /\.ya?ml$/.test(f))
    .map((name) => ({ name, text: readFileSync(join(".github/workflows", name), "utf8") }));
  for (const { name, error } of workflowErrors(workflows)) {
    errors.push(`::error file=.github/workflows/${name},title=a workflow this check does not place::${error} (tools/release-features.mjs)`);
  }

  for (const workflow of SHIPPING_WORKFLOWS) {
    const path = `.github/workflows/${workflow}`;
    const found = builds(readFileSync(path, "utf8"));
    if (found.length === 0) errors.push(`::error file=${path},title=no build read::${workflow} ships a binary, and this check read no build from it`);
    for (const build of found) {
      const at = `${path}:${build.line}`;
      if (build.error) {
        errors.push(`::error file=${path},line=${build.line},title=a release build this check cannot read::${build.error}`);
        continue;
      }
      if (!existsSync(build.manifest)) {
        errors.push(`::error file=${path},line=${build.line},title=a release build this check cannot read::no manifest at ${build.manifest}`);
        continue;
      }
      const meta = metadata(build.manifest);
      const own = meta.packages.find((p) => p.manifest_path === join(process.cwd(), build.manifest));
      const packages = build.packages.length > 0 && !build.manifest.startsWith(TAURI_PROJECT) ? build.packages : [own?.name, ...build.packages];
      if (packages.some((p) => !p)) {
        errors.push(`::error file=${path},line=${build.line},title=a release build this check cannot read::${build.manifest} names no package of its own`);
        continue;
      }
      const lock = join(meta.workspace_root, "Cargo.lock");
      const manifests = new Map(meta.packages.map((p) => [p.name, p.manifest_path]));
      const label = `the build at ${at} (${packages.join(" + ")})`;
      for (const test of TEST_ONLY) {
        if (!readFileSync(lock, "utf8").includes(`name = "${test.crate}"`)) {
          console.log(`${label}: its lock has no ${test.crate}`);
          continue;
        }
        const found = enablers(tree({ manifest: build.manifest, packages, crate: test.crate, edges: "normal,build,features" }), test);
        const lines = annotations({ found, test, build: label, manifests, root: process.cwd() });
        if (lines.length === 0) console.log(`${label}: no ${test.crate}/${test.feature}`);
        errors.push(...lines);
      }
    }
  }
  for (const line of errors) console.log(line);
  return errors.length === 0 ? 0 : 1;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exitCode = main();

