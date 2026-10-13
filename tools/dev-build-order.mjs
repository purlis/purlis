#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Whether a dev build may replace the one the dev channel serves: only a build of a later
// commit, under a later version, does.
//
//   node tools/dev-build-order.mjs --version <version> --sha <commit>
//
// Needs GH_TOKEN and GITHUB_REPOSITORY, as `release.yml`'s `plan` job has them. Prints one
// line, `publish: <why>` or `skip: <why>`, and exits 0; exits 1 when it could not find out,
// so a build never publishes on a guess.
//
// **Why.** `release.yml` builds the commit whose `ci` went green, in the order the `ci` runs
// END, and that is not the order of the commits: a `ci` run started again finishes after its
// successors'. On 2026-10-13 run 654 published 4c7854ee over e827217c, nine commits newer,
// which run 653 had published half an hour before. Its number was the higher one, so every
// machine on the dev channel took it and went back nine commits.
//
// **What is published** is read from `dev.json`, the file the apps themselves read: its
// `version`, and the commit its `notes` name (`release.yml`'s `manifest` job writes "a dev
// build of <sha>"; `tools/release-workflow.test.mjs` holds the two together). How the two
// commits stand is the forge's answer to a comparison, so `plan` needs no history checked out.
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

/** The version and the commit of the build a `dev.json` announces. */
export function publishedBuild(text) {
  let manifest;
  try {
    manifest = JSON.parse(text);
  } catch (error) {
    throw new Error(`the published dev.json is not JSON: ${error.message}`);
  }
  const version = manifest?.version;
  if (typeof version !== "string" || version === "") {
    throw new Error("the published dev.json has no version");
  }
  const sha = /\b[0-9a-f]{40}\b/.exec(String(manifest.notes ?? ""))?.[0];
  if (!sha) {
    throw new Error(
      `the published dev.json (${version}) names no commit in its notes, so this build cannot be placed after it`,
    );
  }
  return { version, sha };
}

/** `1.2.3` or `1.2.3-dev.45`, as its numbers; a release sorts after its own dev builds. */
function numbers(version) {
  const found = /^(\d+)\.(\d+)\.(\d+)(?:-dev\.(\d+))?$/.exec(version);
  if (!found) throw new Error(`${JSON.stringify(version)} is not a version this channel publishes`);
  const [major, minor, patch, dev] = found.slice(1);
  return [Number(major), Number(minor), Number(patch), dev === undefined ? Infinity : Number(dev)];
}

/** Whether an app running `than` would take `version` as an update. */
export function newerVersion(version, than) {
  const [ours, theirs] = [numbers(version), numbers(than)];
  for (let i = 0; i < ours.length; i += 1) {
    if (ours[i] !== theirs[i]) return ours[i] > theirs[i];
  }
  return false;
}

/**
 * Whether the build of `sha`, to be announced as `version`, may replace `published`.
 *
 * `relation` is how `sha` stands to the published commit, in the forge's words for a
 * comparison from the published commit to `sha`: `ahead`, `behind`, `identical` or `diverged`.
 */
export function decide({ published, version, sha, relation }) {
  if (published === null) {
    return { publish: true, why: "the dev channel serves nothing yet" };
  }
  const theirs = `${published.version}, built from ${published.sha}`;
  if (relation === "identical") {
    // Only the run that published it may publish it again: `dev.json` goes up before its
    // weekly twin and the release's note, so a run that died between them can finish.
    return published.version === version
      ? { publish: true, why: `this run published ${version} already and is finishing it` }
      : { publish: false, why: `the dev channel already serves this commit (${theirs})` };
  }
  if (relation === "behind") {
    return {
      publish: false,
      why: `${sha} is older than what the dev channel serves (${theirs}), and the channel only moves forward`,
    };
  }
  if (relation !== "ahead" && relation !== "diverged") {
    throw new Error(
      `the compare of ${published.sha} with ${sha} answered ${JSON.stringify(relation)}, which says nothing about their order`,
    );
  }
  if (!newerVersion(version, published.version)) {
    return {
      publish: false,
      why: `${version} is not newer than what the dev channel serves (${theirs}), so no app would take it; the next merge's build publishes this commit`,
    };
  }
  return relation === "ahead"
    ? { publish: true, why: `${sha} is after what the dev channel serves (${theirs})` }
    : {
        publish: true,
        // Only a rewritten `main` gets here: both commits had a green `ci` as a push to it.
        why: `what the dev channel serves (${theirs}) is not in this commit's history, so main was rewritten and its build publishes`,
      };
}

/**
 * [`decide`], with what is published and how the commits stand read from the forge through
 * `api`: a function from `gh api`'s arguments to what it printed, which throws when it fails.
 */
export function ask({ repo, version, sha, api }) {
  let release;
  try {
    release = JSON.parse(api([`repos/${repo}/releases/tags/dev`]));
  } catch (error) {
    // No `dev` release at all: nothing is published, and `plan`'s next step says how the
    // operator creates it. Any other failure is not an answer.
    if (!/HTTP 404/.test(String(error.message))) throw error;
    return decide({ published: null, version, sha, relation: null });
  }
  const asset = (release.assets ?? []).find((one) => one.name === "dev.json");
  if (!asset) return decide({ published: null, version, sha, relation: null });
  const published = publishedBuild(
    api(["-H", "Accept: application/octet-stream", `repos/${repo}/releases/assets/${asset.id}`]),
  );
  if (published.sha === sha) return decide({ published, version, sha, relation: "identical" });
  const compared = JSON.parse(api([`repos/${repo}/compare/${published.sha}...${sha}`]));
  return decide({ published, version, sha, relation: compared.status });
}

function ghApi(args) {
  try {
    return execFileSync("gh", ["api", ...args], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
      maxBuffer: 16 * 1024 * 1024,
    });
  } catch (error) {
    throw new Error(`gh api ${args.at(-1)}: ${String(error.stderr || error.message).trim()}`);
  }
}

function main(argv) {
  const value = (flag) => {
    const at = argv.indexOf(flag);
    const found = at === -1 ? undefined : argv[at + 1];
    if (!found) throw new Error(`usage: dev-build-order.mjs --version <version> --sha <commit>`);
    return found;
  };
  const repo = process.env.GITHUB_REPOSITORY;
  if (!repo) throw new Error("GITHUB_REPOSITORY is not set");
  const order = ask({ repo, version: value("--version"), sha: value("--sha"), api: ghApi });
  console.log(`${order.publish ? "publish" : "skip"}: ${order.why}`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    console.error(`::error::${error.message}`);
    process.exit(1);
  }
}
