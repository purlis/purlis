// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// No release build carries a crate's test-only feature (#2216). Run with `node --test tools/`.
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { test } from "node:test";

import {
  CANARY,
  SHIPPING_WORKFLOWS,
  TAURI_PROJECT,
  TEST_ONLY,
  annotations,
  builds,
  enablers,
  listErrors,
  tauriConfigFeatures,
  NOT_SHIPPING,
  workflowErrors,
} from "./release-features.mjs";

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
    "release.yml": [
      { manifest: "Cargo.toml", packages: ["purlis-cli", "persona-statistics"] },
      app,
      app,
      // `cargo run --release -p release-manifest --bin sign-runner-image`: signs the runner
      // image's line with the updater's key (#2749).
      { manifest: "Cargo.toml", packages: ["release-manifest"] },
      // `cargo run --release -p release-manifest`: the tool that writes the update manifests.
      { manifest: "Cargo.toml", packages: ["release-manifest"] },
    ],
    "runner-image.yml": [{ manifest: "Cargo.toml", packages: ["purlis-cli"] }],
    "operator-image.yml": [{ manifest: "operator/Cargo.toml", packages: [] }],
  });
});

test("a compiling cargo subcommand this check does not know is an error, not skipped", () => {
  assert.match(one("cargo zigbuild --release -p a").error, /`cargo zigbuild` is not a subcommand this check knows/);
  assert.match(one("cargo auditable zigbuild -p a").error, /`cargo zigbuild` is not a subcommand this check knows/);
  assert.match(one("cargo +1.99.1 xtask dist").error, /`cargo xtask` is not a subcommand this check knows/);
  assert.match(one('notes=$(cargo dist build -p a)').error, /`cargo dist` is not a subcommand this check knows/);
});

test("cargo's subcommands that make nothing someone is given are not builds", () => {
  for (const line of [
    "cargo run --locked --quiet -p changelog -- \"$VERSION\" CHANGELOG.md",
    'if ! notes=$(cargo run --locked --quiet -p changelog -- "$VERSION" CHANGELOG.md 2>&1); then',
    "cargo test --locked",
    "cargo clippy --locked --all-targets",
    "cargo fmt --check",
    "cargo deny --all-features --config ../deny.toml check",
    "cargo audit bin dist/purlis",
    "cargo metadata --format-version 1",
    "cargo tree -e features",
    "cargo --version",
  ]) {
    assert.deepEqual(builds(`      - run: ${line}`), [], line);
  }
});

test("cargo called through a variable or by path is an error", () => {
  assert.match(one('"$CARGO" build -p a').error, /through a variable/);
  assert.match(one("$CARGO build -p a").error, /through a variable/);
  assert.match(one("${CARGO} auditable build -p a").error, /through a variable/);
  assert.match(one("~/.cargo/bin/cargo build -p a").error, /by path/);
  assert.match(one('"$HOME/.cargo/bin/cargo" zigbuild -p a').error, /by path/);
});

test("a workflow that builds is shipping or not shipping, never neither", () => {
  for (const text of [
    "      - run: cargo build --locked -p a",
    "      - run: npx tauri build",
    "      - run: cargo zigbuild -p a",
    "      - uses: tauri-apps/tauri-action@0123456789abcdef0123456789abcdef01234567 # v0",
  ]) {
    assert.deepEqual(workflowErrors([{ name: "new.yml", text }]).map((e) => e.name), ["new.yml"], text);
    assert.match(workflowErrors([{ name: "new.yml", text }])[0].error, /neither SHIPPING_WORKFLOWS nor NOT_SHIPPING/);
    assert.deepEqual(workflowErrors([{ name: SHIPPING_WORKFLOWS[0], text }]), [], text);
  }
  for (const text of ["      - run: cargo build --locked -p a", "      - run: npx tauri build"]) {
    assert.deepEqual(workflowErrors([{ name: Object.keys(NOT_SHIPPING)[0], text }]), [], text);
  }
  assert.deepEqual(workflowErrors([{ name: "docs.yml", text: "      - run: cargo run -p changelog" }]), []);
});

test("each workflow that does not ship says why", () => {
  for (const [name, why] of Object.entries(NOT_SHIPPING)) {
    assert.ok(why.length > 20, name);
    assert.ok(!SHIPPING_WORKFLOWS.includes(name), name);
  }
});

test("a workflow listed as not shipping that publishes is an error", () => {
  const listed = Object.keys(NOT_SHIPPING)[0];
  for (const publish of [
    "      - run: gh release create v1 dist/*",
    "      - uses: docker/build-push-action@0123456789abcdef0123456789abcdef01234567 # v7",
    "    permissions:\n      packages: write",
  ]) {
    const [found] = workflowErrors([{ name: listed, text: `${publish}\n      - run: cargo build -p a` }]);
    assert.equal(found?.name, listed, publish);
    assert.match(found.error, /publishes/);
  }
  assert.deepEqual(workflowErrors([{ name: listed, text: "      # gh release create v1\n      - run: cargo build -p a" }]), []);
});

test("cargo run or test with a release profile is a build, never skipped", () => {
  assert.deepEqual(one("cargo run --locked --release -p release-manifest -- --out x"), {
    line: 1, manifest: "Cargo.toml", packages: ["release-manifest"],
  });
  assert.deepEqual(one("cargo r -r -p a"), { line: 1, manifest: "Cargo.toml", packages: ["a"] });
  assert.deepEqual(one("cargo run --profile dev-release -p a"), { line: 1, manifest: "Cargo.toml", packages: ["a"] });
  assert.match(one("cargo test --release -p a").error, /test build/);
  assert.match(one("cargo t --profile=x -p a").error, /test build/);
  assert.match(one("cargo test --no-run -p a").error, /test build/);
  assert.deepEqual(builds('      - run: notes=$(cargo run --locked --quiet -p changelog -- "$VERSION" CHANGELOG.md)'), []);
});

test("cargo inside a quoted command, a plugin's own binary or cargo.exe is a build this check cannot read", () => {
  assert.match(one("sh -c 'cargo build --release -p a'").error, /after other words/);
  assert.match(one('bash -c "cargo build -p a"').error, /after other words/);
  assert.match(one("cargo-zigbuild build -p a").error, /plugin/);
  assert.match(one("cd x && cargo-auditable auditable build -p a").error, /plugin/);
  assert.match(one("cargo.exe build -p a").error, /by path/);
  assert.deepEqual(builds("      - name: cargo-auditable"), []);
  assert.deepEqual(builds("      - run: cargo install --locked cargo-auditable --version \"$CARGO_AUDITABLE_VERSION\""), []);
});

test("cargo through any variable expansion, or a variable named for cargo, is an error", () => {
  assert.match(one("${CARGO:-cargo} build -p a").error, /through a variable/);
  assert.match(one('"$CARGO" zigbuild -p a').error, /through a variable/);
  assert.match(one("$cargo nextest run").error, /through a variable/);
  assert.match(one("cd x && $MY_CARGO xtask dist").error, /through a variable/);
});

test("every workflow today that ships a build is listed", () => {
  const dir = new URL("../.github/workflows/", import.meta.url);
  const workflows = readdirSync(dir)
    .filter((f) => /\.ya?ml$/.test(f))
    .map((name) => ({ name, text: readFileSync(new URL(name, dir), "utf8") }));
  assert.ok(workflows.length > SHIPPING_WORKFLOWS.length);
  assert.deepEqual(workflowErrors(workflows), []);
});

// What `cargo metadata --no-deps` says of the workspace's packages, cut to what `listErrors`
// reads: each package's name and its features.
const workspace = [
  { name: "purlis-session-protocol", features: { "any-stream": [], bench: ["tokio/rt-multi-thread"] } },
  { name: "purlis-core", features: { fenced: [], "test-support": [] } },
  { name: "purlis-host", features: { "test-support": [] } },
  { name: "purlis-skills", features: { "test-support": [] } },
  { name: "purlis-terminal", features: { "test-support": [] } },
  { name: "purlis-app", features: { e2e: ["purlis-core/fenced"] } },
  { name: "purlis-cli", features: {} },
];

test("every test-support feature in the workspace is listed as test-only, each with a canary", () => {
  assert.deepEqual(listErrors(workspace), []);
  for (const { crate, feature } of TEST_ONLY) {
    assert.ok(CANARY[`${crate}/${feature}`], `${crate}/${feature} has no canary`);
  }
});

test("a test-support feature the list leaves out is an error, not a feature the resolver alone keeps out", () => {
  const withANewOne = [...workspace, { name: "purlis-browser-engine", features: { "test-support": [] } }];
  assert.deepEqual(listErrors(withANewOne), [
    "purlis-browser-engine/test-support is a test-only feature TEST_ONLY does not list: list it, with a CANARY",
  ]);
});

test("a listed feature no package has any more, or one with no canary, is an error", () => {
  const gone = workspace.filter((p) => p.name !== "purlis-skills");
  assert.deepEqual(listErrors(gone), ["TEST_ONLY lists purlis-skills/test-support, and no package of the workspace has it"]);
  const unwatched = [...TEST_ONLY, { crate: "purlis-cli", feature: "test-support" }];
  const withIt = workspace.map((p) => (p.name === "purlis-cli" ? { ...p, features: { "test-support": [] } } : p));
  assert.deepEqual(listErrors(withIt, unwatched), ["TEST_ONLY lists purlis-cli/test-support with no CANARY to show this check still sees it"]);
});

test("a release build that turns on any listed test-only feature is an error on the manifest that did", () => {
  const manifests = new Map([["purlis-cli", "/w/crates/purlis-cli/Cargo.toml"]]);
  for (const test of TEST_ONLY) {
    // `purlis-cli`'s [dependencies] asking for the feature, as `cargo tree -i` prints it.
    const tree = [
      `0${test.crate} v0.4.2 (/w/crates/x)`,
      `1${test.crate} feature "${test.feature}"`,
      "2purlis-cli v0.4.2 (/w/crates/purlis-cli)",
      '3purlis-cli feature "default" (command-line)',
      `1${test.crate} feature "default"`,
      "2purlis-cli v0.4.2 (/w/crates/purlis-cli) (*)",
    ].join("\n");
    const found = enablers(tree, test);
    assert.deepEqual(found, [{ name: "purlis-cli", via: null, askedBy: [] }], test.feature);
    assert.deepEqual(annotations({ found, test, build: "the build at release.yml:9 (purlis-cli)", manifests, root: "/w" }), [
      "::error file=crates/purlis-cli/Cargo.toml,title=test-only feature in a release build::" +
        `purlis-cli turns on ${test.crate}'s \`${test.feature}\` in its [dependencies], ` +
        "so the build at release.yml:9 (purlis-cli) carries it. Ask for it from [dev-dependencies] only.",
    ]);
  }
});
