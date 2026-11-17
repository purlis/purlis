// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Which runner assets a dev publish removes from the `dev` release (#723): those of builds
// older than the last five, so a desktop a dev build or a few behind still finds its own
// version's runner binary at `runnerinstall::asset_url`. Run with `node --test tools/`.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { KEPT, staleRunnerAssets } from "./dev-runner-assets.mjs";

const TARGETS = ["linux-x86_64", "linux-aarch64", "macos-arm64"];

/** The six assets of one dev build: three binaries and their signatures. */
function build(version, from) {
  return TARGETS.flatMap((target, at) => [
    { id: from + 2 * at, name: `purlis-${version}-${target}` },
    { id: from + 2 * at + 1, name: `purlis-${version}-${target}.sig` },
  ]);
}

/** The release's assets after a publish of `version` removed what it names. */
function after(assets, version) {
  const gone = new Set(staleRunnerAssets(assets, version));
  return assets.filter((asset) => !gone.has(asset.id));
}

test("a desktop one dev build behind still finds its runner asset after the next publish", () => {
  const behind = "0.9.0-dev.811";
  const now = "0.9.0-dev.812";
  const left = after([...build(behind, 100), ...build(now, 200)], now);
  for (const target of TARGETS) {
    // The name `runnerinstall::asset_url` asks the `dev` release for, and its signature.
    for (const name of [`purlis-${behind}-${target}`, `purlis-${behind}-${target}.sig`]) {
      assert.ok(
        left.some((asset) => asset.name === name),
        `${name} is still there`,
      );
    }
  }
});

test("the last five dev builds' runner assets are kept and older ones are removed", () => {
  assert.equal(KEPT, 5);
  const runs = [801, 805, 806, 809, 810, 811, 812];
  const assets = runs.flatMap((run, at) => build(`0.9.0-dev.${run}`, 1000 * (at + 1)));
  const left = after(assets, "0.9.0-dev.812");
  const versions = [...new Set(left.map((asset) => asset.name.split("-").slice(1, 3).join("-")))];
  assert.deepEqual(versions.sort(), [
    "0.9.0-dev.806",
    "0.9.0-dev.809",
    "0.9.0-dev.810",
    "0.9.0-dev.811",
    "0.9.0-dev.812",
  ]);
  assert.equal(left.length, 5 * 6);
});

test("the build being published is never removed, nor anything that is not a runner asset", () => {
  // A run started again by hand publishes an older build than one already there.
  const assets = [
    ...build("0.9.0-dev.900", 1),
    ...build("0.9.0-dev.901", 100),
    { id: 7001, name: "dev.json" },
    { id: 7002, name: "purlis_0.9.0-dev.900_amd64.deb" },
    { id: 7003, name: "purlis-runner-image.txt" },
  ];
  const gone = new Set(staleRunnerAssets(assets, "0.9.0-dev.900", 1));
  for (const asset of build("0.9.0-dev.900", 1)) assert.ok(!gone.has(asset.id), asset.name);
  for (const id of [7001, 7002, 7003]) assert.ok(!gone.has(id));
});

test("a runner asset of no dev version is removed from the dev release", () => {
  const assets = [...build("0.9.0", 1), ...build("0.9.0-dev.3", 100)];
  const gone = new Set(staleRunnerAssets(assets, "0.9.0-dev.3"));
  assert.deepEqual(
    [...gone].sort((a, b) => a - b),
    build("0.9.0", 1).map((asset) => asset.id),
  );
});

test("as a command it reads the release's assets and prints the ids to remove", () => {
  const assets = [...build("0.9.0-dev.1", 1), ...build("0.9.0-dev.7", 100)];
  const printed = execFileSync(
    process.execPath,
    [fileURLToPath(new URL("./dev-runner-assets.mjs", import.meta.url)), "0.9.0-dev.7", "1"],
    { input: JSON.stringify(assets) },
  ).toString();
  assert.deepEqual(
    printed.trim().split("\n").map(Number),
    build("0.9.0-dev.1", 1).map((asset) => asset.id),
  );
});
