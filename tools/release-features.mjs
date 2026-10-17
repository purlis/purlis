// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// No release build carries a crate's test-only feature (#2216).
//
// `purlis-session-protocol`'s `any-stream` holds its test-only stand-ins (a link served over
// any stream, a device taken as proved, the link's stand-in handshake). Its own tests and its
// users' tests ask for it from `[dev-dependencies]`, which Cargo leaves out of a build that is
// not a test. This reads every build the shipping workflows make, asks Cargo which features
// that build resolves for the crate (`cargo tree -e normal,build,features`, every target,
// nothing compiled), and fails with an error on the manifest that turned the feature on.
//
// It fails closed. A build line it cannot read whole, a feature flag on one, a shipping
// workflow with no build it can read, `cargo tree` output it cannot parse, or a canary that no
// longer finds the feature where a dev-dependency does turn it on: each is an error, so a
// change in how a binary is built or in what Cargo prints cannot quietly turn the check off.
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

/** The features only a test may turn on. */
export const TEST_ONLY = [{ crate: "purlis-session-protocol", feature: "any-stream" }];

/** A crate whose `[dev-dependencies]` turns each test-only feature on: the canary's build. */
export const CANARY = { "purlis-session-protocol/any-stream": "purlis-host" };

/** The Tauri project `tauri build` builds, run from `app/`. */
export const TAURI_PROJECT = "app/src-tauri";

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
  ["--release", false], ["--locked", false], ["--frozen", false], ["--offline", false],
  ["--bins", false], ["-v", false], ["--verbose", false], ["-q", false], ["--quiet", false],
]);
/** The cargo subcommands that build. */
const BUILDS = ["build", "b", "rustc"];
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

/** One build line's words, shell assignments in front dropped; null when it is not a command. */
function words(command) {
  const tokens = command.replace(/\$\{\{[^}]*\}\}/g, "EXPR").trim().split(/\s+/);
  while (tokens.length > 0 && /^[A-Za-z_][A-Za-z0-9_]*=/.test(tokens[0])) tokens.shift();
  return tokens;
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
    const cargo = /(^|[^\w-])cargo\s/.test(text);
    const tauri = /\btauri\s+build\b/.test(text);
    if (!cargo && !tauri) continue;
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
    // Each `cargo` word and the subcommand after it (past a `+toolchain` and `auditable`).
    const calls = tokens.flatMap((token, k) => {
      if (token.replace(/^\$?\(+/, "") !== "cargo") return [];
      let at = k + 1;
      if (tokens[at]?.startsWith("+")) at++;
      if (tokens[at] === "auditable") at++;
      return [{ k, at, sub: tokens[at] }];
    });
    const building = calls.filter(({ sub, at }) => BUILDS.includes(sub) || (sub === "install" && tokens.slice(at).some((t) => /^--(path|git)(=|$)/.test(t))));
    if (building.length === 0) continue;
    const [{ k, at, sub }] = building;
    if (k !== 0 || building.length > 1) {
      found.push({ line, error: "a cargo build after other words is not one this check reads" });
      continue;
    }
    if (sub === "install") {
      found.push({ line, error: "`cargo install` from a path or a git repository is a build this check does not read" });
      continue;
    }
    const into = { manifest: "Cargo.toml", packages: [] };
    const error = cargoArgs(tokens.slice(at + 1), into);
    if (error) found.push({ line, error });
    else if (into.manifest === "Cargo.toml" && into.packages.length === 0) {
      found.push({ line, error: "a build of the root workspace with no `-p` builds its default members: name the packages" });
    } else found.push({ line, ...into });
  }
  return found;
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

  for (const test of TEST_ONLY) {
    const canary = CANARY[`${test.crate}/${test.feature}`];
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

