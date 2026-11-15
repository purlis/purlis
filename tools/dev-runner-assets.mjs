#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Which runner assets a dev publish removes from the `dev` release (#723).
//
//   gh api --paginate repos/<repo>/releases/tags/dev --jq '[.assets[] | {id, name}]' \
//     | node tools/dev-runner-assets.mjs <version being published> [<builds kept>]
//
// Prints the id of each asset to remove, one a line, and nothing else.
//
// **Why not only this build's.** A runner asset, `purlis-<version>-<target>` and its `.sig`, is
// named by its version, and a desktop installs on a runner the version it is itself
// (`purlis_core::runnerinstall::asset_url`). Keeping only the newest build's would leave every
// desktop one dev build behind with nothing to fetch until it updated. So the `dev` release keeps
// the runner assets of the last `KEPT` dev builds, counted by their run number, the build being
// published always among them, and removes the rest: an address once published names the same
// bytes for as long as it answers. Not a tag per build: `release.yml` creates, moves and deletes
// no tag. A runner asset of a version that is not a dev build's has no place on the `dev`
// release and is removed. Anything else on it is not this file's.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

/** How many dev builds' runner assets the `dev` release keeps. */
export const KEPT = 5;

const RUNNER = /^purlis-(.+)-(?:linux-x86_64|linux-aarch64|macos-arm64)(?:\.sig)?$/;
const DEV = /^.+-dev\.(\d+)$/;

/**
 * The ids of the runner assets in `assets` (each `{id, name}`) that a publish of `version`
 * removes, keeping those of the newest `kept` dev builds and always `version`'s own.
 */
export function staleRunnerAssets(assets, version, kept = KEPT) {
  const run = (v) => Number(DEV.exec(v)?.[1] ?? NaN);
  const runners = assets
    .map((asset) => ({ ...asset, version: RUNNER.exec(asset.name)?.[1] }))
    .filter((asset) => asset.version !== undefined);
  const newest = [...new Set(runners.map((asset) => asset.version).filter((v) => DEV.test(v)))]
    .filter((v) => v !== version)
    .sort((a, b) => run(b) - run(a))
    .slice(0, Math.max(kept - 1, 0));
  const keep = new Set([version, ...newest]);
  return runners.filter((asset) => !keep.has(asset.version)).map((asset) => asset.id);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [version, kept] = process.argv.slice(2);
  if (!version) {
    console.error("usage: node tools/dev-runner-assets.mjs <version> [<builds kept>] < assets.json");
    process.exit(2);
  }
  const assets = JSON.parse(readFileSync(0, "utf8"));
  for (const id of staleRunnerAssets(assets, version, kept === undefined ? KEPT : Number(kept))) {
    console.log(id);
  }
}
