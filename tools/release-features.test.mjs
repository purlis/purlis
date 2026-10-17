// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// No release build carries a crate's test-only feature (#2216). Run with `node --test tools/`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { SHIPPING_WORKFLOWS, TAURI_PROJECT, annotations, builds, enablers, tauriConfigFeatures } from "./release-features.mjs";

// Written by hand, in the shape `cargo tree --prefix depth -i purlis-session-protocol -e
// normal,build,features -p purlis-cli` printed on 2026-10-17 (paths shortened to `/w`): one
// as on main, and one with `purlis-host`'s `[dependencies]` line asking for `any-stream`.
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

test("a release build without the feature has nothing to name", () => {
  assert.deepEqual(enablers(clean, FEATURE), []);
});

test("a dependency line that asks for the feature names its crate, and the crate further up", () => {
  assert.deepEqual(enablers(fromADependencyLine, FEATURE), [
    { name: "purlis-host", via: null, askedBy: ["purlis-cli"] },
  ]);
});

test("a feature of another crate that turns it on names that crate, its feature and who asked", () => {
  const tree = [
    "0purlis-session-protocol v0.4.2 (/w/crates/session-protocol)",
    '1purlis-session-protocol feature "any-stream"',
    '2purlis-core feature "serve"',
    '3purlis-core feature "default"',
    "4purlis-cli v0.4.2 (/w/crates/purlis-cli)",
    '5purlis-cli feature "default" (command-line)',
  ].join("\n");
  assert.deepEqual(enablers(tree, FEATURE), [
    { name: "purlis-core", via: "serve", askedBy: ["purlis-core feature `default`", "purlis-cli"] },
  ]);
});

test("output of another shape is an error, not an empty answer", () => {
  assert.throws(() => enablers("", FEATURE), /not rooted at purlis-session-protocol/);
  assert.throws(() => enablers("purlis-session-protocol v0.4.2 (/w)\n└── x", FEATURE), /cannot read/);
  assert.throws(() => enablers("0purlis-session-protocol v0.4.2 (/w)\n1purlis-host v0.4.2 (/w)", FEATURE), /no feature node/);
  assert.throws(() => enablers("0purlis-core v0.4.2 (/w)\n1purlis-core feature \"default\"", FEATURE), /not rooted/);
});

test("each crate that turns it on is an error on its own manifest, naming who asked", () => {
  const manifests = new Map([
    ["purlis-host", "/w/crates/host/Cargo.toml"],
    ["purlis-core", "/w/crates/purlis-core/Cargo.toml"],
  ]);
  const found = [
    { name: "purlis-host", via: null, askedBy: ["purlis-cli"] },
    { name: "purlis-core", via: "serve", askedBy: [] },
  ];
  assert.deepEqual(annotations({ found, test: FEATURE, build: "the build at x.yml:3 (purlis-cli)", manifests, root: "/w" }), [
    "::error file=crates/host/Cargo.toml,title=test-only feature in a release build::" +
      "purlis-host turns on purlis-session-protocol's `any-stream` in its [dependencies] (asked for by purlis-cli), " +
      "so the build at x.yml:3 (purlis-cli) carries it. Ask for it from [dev-dependencies] only.",
    "::error file=crates/purlis-core/Cargo.toml,title=test-only feature in a release build::" +
      "purlis-core turns on purlis-session-protocol's `any-stream` from its feature `serve`, " +
      "so the build at x.yml:3 (purlis-cli) carries it. Ask for it from [dev-dependencies] only.",
  ]);
});

/** The one build a single `run:` line makes. */
function one(line) {
  const found = builds(`      - run: ${line}`);
  assert.equal(found.length, 1, JSON.stringify(found));
  return found[0];
}

test("a cargo build's packages are read in each spelling Cargo takes", () => {
  const read = { line: 1, manifest: "Cargo.toml", packages: ["a", "b", "c", "d"] };
  assert.deepEqual(one('cargo auditable build --profile "$PROFILE" --locked -p a --package b -pc --package=d'), read);
  assert.deepEqual(one("CARGO_TARGET_DIR=t cargo +1.99.1 build --release --target=x86_64 -p a -p b -p c -p d"), read);
  assert.deepEqual(one("cargo build --release --locked --manifest-path operator/Cargo.toml"), {
    line: 1, manifest: "operator/Cargo.toml", packages: [],
  });
});

test("a build line that goes on past a backslash is read whole", () => {
  const found = builds(["      run: |", "        cargo build --locked \\", "          -p a \\", "          -p b"].join("\n"));
  assert.deepEqual(found, [{ line: 2, manifest: "Cargo.toml", packages: ["a", "b"] }]);
});

test("a feature flag on a cargo build is an error", () => {
  for (const flags of ["--features x", "--features=x", "-F x", "-Fx", "--all-features", "--no-default-features"]) {
    assert.match(one(`cargo build -p a ${flags}`).error, /feature flag/, flags);
  }
});

test("a cargo build this check cannot read whole is an error", () => {
  assert.match(one("cargo build --locked").error, /default members/);
  assert.match(one("cargo build --workspace").error, /not a flag this check reads/);
  assert.match(one("cargo build -p a --exclude b").error, /not a flag this check reads/);
  assert.match(one("cargo build -p a 2>&1 | tail").error, /not a flag this check reads/);
  assert.match(one("cargo build -p").error, /has no value/);
  assert.match(one("(cd x && cargo build -p a)").error, /after other words/);
  assert.match(one("cargo install --locked --path crates/purlis-cli").error, /from a path/);
});

test("installing a published tool is not a build of this repository's, and comments are skipped", () => {
  assert.deepEqual(builds("      run: cargo install --locked cargo-auditable --version 0.7.6"), []);
  assert.deepEqual(builds("      # `cargo build -p x` once made this"), []);
});

test("a tauri build is the app's crate, plus what it passes Cargo after --", () => {
  const app = `${TAURI_PROJECT}/Cargo.toml`;
  assert.deepEqual(
    one('npx tauri build --config src-tauri/tauri.build.conf.json --bundles ${{ matrix.bundles }},${{ matrix.extra }} --runner "$R" -- --profile "$PROFILE"'),
    { line: 1, manifest: app, packages: [] },
  );
  assert.deepEqual(one("npx tauri build --debug --no-bundle -- -p fake-harness"), { line: 1, manifest: app, packages: ["fake-harness"] });
});

test("a feature flag on a tauri build, or a flag it does not read, is an error", () => {
  assert.match(one("npx tauri build --features e2e").error, /feature flag/);
  assert.match(one("npx tauri build -f e2e").error, /feature flag/);
  assert.match(one("npx tauri build -- --features e2e").error, /feature flag/);
  assert.match(one("npx tauri build --something").error, /not a flag this check reads/);
  assert.match(one("npm run tauri build").error, /not a tauri build this check reads/);
});

test("build.features in a Tauri config is read", () => {
  assert.deepEqual(tauriConfigFeatures({ build: { beforeBuildCommand: "x" } }), []);
  assert.deepEqual(tauriConfigFeatures({ build: { features: ["e2e"] } }), ["e2e"]);
});

test("the shipping workflows' builds, as read, one by one", () => {
  const read = Object.fromEntries(
    SHIPPING_WORKFLOWS.map((w) => [
      w,
      builds(readFileSync(new URL(`../.github/workflows/${w}`, import.meta.url), "utf8")).map(({ line, ...b }) => b),
    ]),
  );
  const app = { manifest: `${TAURI_PROJECT}/Cargo.toml`, packages: [] };
  assert.deepEqual(read, {
    "release.yml": [{ manifest: "Cargo.toml", packages: ["purlis-cli", "persona-statistics"] }, app, app],
    "runner-image.yml": [{ manifest: "Cargo.toml", packages: ["purlis-cli"] }],
    "operator-image.yml": [{ manifest: "operator/Cargo.toml", packages: [] }],
  });
});
