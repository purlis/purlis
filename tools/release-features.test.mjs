// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// No release build carries a crate's test-only feature (#2216). Run with `node --test tools/`.
//
// The trees below are what `cargo tree --prefix depth -i purlis-session-protocol -e
// normal,build,features` printed for `-p purlis-cli`: one on main, and one with `purlis-host`'s
// `[dependencies]` line asking for `any-stream`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { RELEASE_BUILDS, annotations, enablers, releasedPackages } from "./release-features.mjs";

const ROOT = "/w";
const clean = [
  "0purlis-session-protocol v0.4.2 (/w/crates/session-protocol)",
  '1purlis-session-protocol feature "default"',
  "2purlis-core v0.4.2 (/w/crates/purlis-core)",
  '3purlis-core feature "default"',
  "4purlis-cli v0.4.2 (/w/crates/purlis-cli)",
  '5purlis-cli feature "default" (command-line)',
  "2purlis-host v0.4.2 (/w/crates/host)",
  '3purlis-host feature "default"',
  "4purlis-cli v0.4.2 (/w/crates/purlis-cli) (*)",
].join("\n");

const fromADependencyLine = [
  "0purlis-session-protocol v0.4.2 (/w/crates/session-protocol)",
  '1purlis-session-protocol feature "any-stream"',
  "2purlis-host v0.4.2 (/w/crates/host)",
  '3purlis-host feature "default"',
  "4purlis-cli v0.4.2 (/w/crates/purlis-cli)",
  '5purlis-cli feature "default" (command-line)',
  "4purlis-core v0.4.2 (/w/crates/purlis-core)",
  '5purlis-core feature "default"',
  "6purlis-cli v0.4.2 (/w/crates/purlis-cli) (*)",
  '1purlis-session-protocol feature "default"',
  "2purlis-core v0.4.2 (/w/crates/purlis-core) (*)",
  "2purlis-host v0.4.2 (/w/crates/host) (*)",
].join("\n");

const FEATURE = { crate: "purlis-session-protocol", feature: "any-stream" };
const manifests = new Map([
  ["purlis-host", "/w/crates/host/Cargo.toml"],
  ["purlis-core", "/w/crates/purlis-core/Cargo.toml"],
]);

test("a release build without the feature has nothing to name", () => {
  assert.deepEqual(enablers(clean, FEATURE), []);
});

test("a dependency line that asks for the feature names the crate that wrote it", () => {
  assert.deepEqual(enablers(fromADependencyLine, FEATURE), [{ name: "purlis-host", via: null }]);
});

test("a feature of another crate that turns it on names that crate and its feature", () => {
  const tree = [
    "0purlis-session-protocol v0.4.2 (/w/crates/session-protocol)",
    '1purlis-session-protocol feature "any-stream"',
    '2purlis-core feature "serve"',
    '3purlis-core feature "default"',
    "4purlis-cli v0.4.2 (/w/crates/purlis-cli)",
  ].join("\n");
  assert.deepEqual(enablers(tree, FEATURE), [{ name: "purlis-core", via: "serve" }]);
});

test("each crate that turns it on is an error on its own manifest", () => {
  const found = [
    { name: "purlis-host", via: null },
    { name: "purlis-core", via: "serve" },
  ];
  assert.deepEqual(annotations(found, FEATURE, ["purlis-cli"], manifests, ROOT), [
    "::error file=crates/host/Cargo.toml,title=test-only feature in a release build::" +
      "purlis-host turns on purlis-session-protocol's `any-stream` in its [dependencies], " +
      "so the release build of purlis-cli carries it. Ask for it from [dev-dependencies] only.",
    "::error file=crates/purlis-core/Cargo.toml,title=test-only feature in a release build::" +
      "purlis-core turns on purlis-session-protocol's `any-stream` from its feature `serve`, " +
      "so the release build of purlis-cli carries it. Ask for it from [dev-dependencies] only.",
  ]);
});

test("the builds checked are the ones release.yml makes", () => {
  const release = readFileSync(new URL("../.github/workflows/release.yml", import.meta.url), "utf8");
  assert.deepEqual(releasedPackages(release).sort(), RELEASE_BUILDS.flat().sort());
});

test("the release workflow's packages are read from its cargo build lines and the app's tauri build", () => {
  const yml = [
    "      - run: cargo auditable build --profile \"$PROFILE\" --locked -p purlis-cli -p persona-statistics",
    "      - run: npx tauri build --config src-tauri/tauri.build.conf.json -- --profile \"$PROFILE\"",
  ].join("\n");
  assert.deepEqual(releasedPackages(yml), ["purlis-cli", "persona-statistics", "purlis-app"]);
});
