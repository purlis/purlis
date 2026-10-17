// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// No release build carries a crate's test-only feature (#2216).
//
// `purlis-session-protocol`'s `any-stream` holds its test-only stand-ins (a link served over
// any stream, a device taken as proved, the link's stand-in handshake). Its own tests and its
// users' tests ask for it from `[dev-dependencies]`, which Cargo leaves out of a build that is
// not a test. This asks Cargo, without compiling anything, which features each release build
// resolves for the crate (`cargo tree -e normal,build,features`, every target), and fails with
// an error on the manifest that turned the feature on.
//
// Run by the `lint` job from the repository root: `node tools/release-features.mjs`.

import { execFileSync } from "node:child_process";
import { relative } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

/** Each release build, as the packages one `cargo build` names (release.yml). */
export const RELEASE_BUILDS = [["purlis-cli", "persona-statistics"], ["purlis-app"]];

/** The features only a test may turn on. */
export const TEST_ONLY = [{ crate: "purlis-session-protocol", feature: "any-stream" }];

/** One line of `cargo tree --prefix depth`: its depth and what follows. */
function parse(line) {
  const match = /^(\d+)(.*)$/.exec(line);
  return match ? { depth: Number(match[1]), text: match[2] } : null;
}

/**
 * The crates that turn `feature` on, from an inverted `cargo tree --prefix depth` of its crate:
 * each node right under the feature's own. A package node is a dependency line in that crate's
 * manifest (`via: null`); a feature node is one of that crate's features (`via: <name>`).
 */
export function enablers(tree, { crate, feature }) {
  const own = `${crate} feature "${feature}"`;
  const found = [];
  let under = null;
  for (const node of tree.split("\n").map(parse)) {
    if (!node) continue;
    if (under !== null && node.depth <= under) under = null;
    if (under === null) {
      if (node.text.trim() === own) under = node.depth;
      continue;
    }
    if (node.depth !== under + 1) continue;
    const asFeature = /^(\S+) feature "([^"]+)"/.exec(node.text);
    const name = asFeature ? asFeature[1] : node.text.split(" ")[0];
    const via = asFeature ? asFeature[2] : null;
    if (!found.some((f) => f.name === name && f.via === via)) found.push({ name, via });
  }
  return found;
}

/** An `::error` line on each enabling crate's manifest. */
export function annotations(found, { crate, feature }, build, manifests, root) {
  return found.map(({ name, via }) => {
    const manifest = manifests.get(name);
    const file = manifest ? relative(root, manifest) : "Cargo.toml";
    const where = via === null ? "in its [dependencies]" : `from its feature \`${via}\``;
    return (
      `::error file=${file},title=test-only feature in a release build::` +
      `${name} turns on ${crate}'s \`${feature}\` ${where}, ` +
      `so the release build of ${build.join(" + ")} carries it. Ask for it from [dev-dependencies] only.`
    );
  });
}

/** The packages release.yml builds: each `cargo … build`'s `-p`s, and the app for `tauri build`. */
export function releasedPackages(releaseYml) {
  const packages = [];
  for (const line of releaseYml.split("\n")) {
    if (line.trim().startsWith("#")) continue;
    if (/\bcargo (auditable )?build\b/.test(line)) {
      for (const match of line.matchAll(/(?:^|\s)-p\s+(\S+)/g)) packages.push(match[1]);
    }
    if (/\btauri build\b/.test(line) && !packages.includes("purlis-app")) packages.push("purlis-app");
  }
  return [...new Set(packages)];
}

function cargo(args) {
  return execFileSync("cargo", args, { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
}

function main() {
  const metadata = JSON.parse(cargo(["metadata", "--locked", "--no-deps", "--format-version", "1"]));
  const manifests = new Map(metadata.packages.map((p) => [p.name, p.manifest_path]));
  const root = metadata.workspace_root;
  const errors = [];
  for (const build of RELEASE_BUILDS) {
    for (const test of TEST_ONLY) {
      const tree = cargo([
        "tree", "--locked", "--prefix", "depth", "--target", "all",
        "-e", "normal,build,features", "-i", test.crate,
        ...build.flatMap((p) => ["-p", p]),
      ]);
      const lines = annotations(enablers(tree, test), test, build, manifests, root);
      if (lines.length === 0) console.log(`${build.join(" + ")}: no ${test.crate}/${test.feature}`);
      errors.push(...lines);
    }
  }
  for (const line of errors) console.log(line);
  return errors.length === 0 ? 0 : 1;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exitCode = main();
