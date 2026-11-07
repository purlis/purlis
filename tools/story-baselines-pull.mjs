// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **A pull request's baselines, taken from its own `stories` run** (`docs/component-stories.md`):
// the pictures the pinned image drew of the stories, written into this checkout so they go into
// the next push like any other change.
//
//   node tools/story-baselines-pull.mjs <pull request> [--repo <owner>/<repo>]
//
// The repository is purlis/purlis unless `--repo` names another: never gh's default, which in a
// fork's clone is the fork.
//
// When a pull request's `stories` run fails on its pictures alone, the run draws them again,
// writing the baselines as they are now, and keeps them as `story-baselines`
// (`.github/workflows/stories.yml`). This takes that artifact from the newest `stories` run of
// the pull request's head and writes the pictures that differ into `app/` with
// `tools/story-baselines.mjs take`. The
// same check the `update-baselines` label's job does is done first: this checkout is the head
// that run tested, and that head has every commit of the base the run merged it with, so the
// pictures are of this tree. The person then reviews the pictures, commits them and pushes. A
// push of theirs starts every check as usual, where the label's commit, made with the
// workflow's own token, leaves them waiting for a maintainer.
//
// **This runs the checked-out head's own code**, `tools/story-baselines.mjs` among it: take the
// baselines of someone else's pull request only in a checkout of code you would run anyway.
//
// It needs `gh`, signed in, and `git`.
import { spawnSync } from "node:child_process";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  realpathSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, isAbsolute, join, parse, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { take } from "./story-baselines.mjs";

/** The repository a pull request is in, unless `--repo` names another. */
export const REPO = "purlis/purlis";

/** The artifact a `stories` run keeps its drawn baselines in, and the file that says of what. */
export const ARTIFACT = "story-baselines";
export const DRAWN = "drawn.json";

/**
 * The run whose baselines are taken, of `runs` (the `stories` workflow's runs of the head, as
 * the API lists them), or why there is none: the newest run the pull request started, of `head`
 * in the repository `from` its branch is in, which must have finished and failed (one that
 * passed drew nothing; one that is still going has not drawn yet). Only a `pull_request` run
 * draws baselines.
 */
export function runToTake(runs, head, from) {
  const newest = [...runs]
    .filter(
      (run) =>
        run.head_sha === head &&
        run.event === "pull_request" &&
        run.head_repository?.full_name === from,
    )
    .sort((a, b) => b.run_number - a.run_number || b.run_attempt - a.run_attempt)[0];
  if (!newest) return { why: `no stories run has tested ${head} yet` };
  if (newest.status !== "completed")
    return { why: `the stories run of ${head} has not finished: ${newest.html_url}` };
  if (newest.conclusion === "success")
    return {
      why: `the stories run of ${head} passed, so every baseline already matches: ${newest.html_url}`,
    };
  return { run: newest };
}

/** Why the baselines `drawn` (the artifact's `drawn.json`) are not of `head`, or `undefined`. */
export function notOf(drawn, head) {
  if (typeof drawn !== "object" || drawn === null || typeof drawn.head !== "string")
    return `${DRAWN} does not say which commit it drew`;
  if (drawn.head !== head) return `the baselines were drawn of ${drawn.head}, not of ${head}`;
  if (typeof drawn.base !== "string" || !/^[0-9a-f]{40}$/.test(drawn.base))
    return `${DRAWN} does not say which base the head was merged with`;
  return undefined;
}

/** A pull's own download folder: `mkdtempSync` adds six characters to `story-baselines-`. */
const LEFTOVER = /^story-baselines-[A-Za-z0-9]{6}$/;

/** How long nothing has written to a pull's folder before it is taken for a killed pull's. */
export const IDLE_MS = 24 * 60 * 60 * 1000;

/** The newest time anything under `path` was written, not following links. */
const newestWrite = (path) => {
  const stat = lstatSync(path);
  let newest = stat.mtimeMs;
  if (stat.isDirectory())
    for (const name of readdirSync(path)) newest = Math.max(newest, newestWrite(join(path, name)));
  return newest;
};

/**
 * Removes the `story-baselines-*` folders directly under `tmp` that nothing has written to for
 * `idle` milliseconds before `now`, answering the paths it removed (#5348). A pull removes its
 * folder in a `finally`, which a killed pull never reaches, and the temp folder is shared, so
 * only age says a folder is a leftover: a pull running alongside writes to its own all along.
 *
 * Only a real folder (never a link) whose name is exactly a pull's own, directly in the resolved
 * `tmp`, is removed; a `tmp` that resolves to nothing, or to a root, clears nothing.
 */
export function clearLeftovers({ tmp = tmpdir(), now = Date.now(), idle = IDLE_MS } = {}) {
  if (typeof tmp !== "string" || tmp.trim() === "") return [];
  let at;
  try {
    at = realpathSync(tmp);
  } catch {
    return [];
  }
  if (!at || !isAbsolute(at) || parse(at).root === at) return [];
  const removed = [];
  for (const entry of readdirSync(at, { withFileTypes: true })) {
    if (!entry.isDirectory() || !LEFTOVER.test(entry.name)) continue;
    const path = join(at, entry.name);
    if (dirname(path) !== at) continue;
    try {
      if (!lstatSync(path).isDirectory()) continue;
      if (now - newestWrite(path) < idle) continue;
      rmSync(path, { recursive: true, force: true });
      removed.push(path);
    } catch {
      // Gone already, or not ours to read: another pull's business.
    }
  }
  return removed;
}

/** Why a pull took nothing: said to the person, and the tool exits 1. */
export class Refused extends Error {}

/**
 * Takes pull request `pr`'s baselines from its newest failed `stories` run in `repo` and writes
 * them into `root`'s `app/`, answering the lines that say what it did; throws `Refused` saying
 * why it took nothing. `exec` runs a command as `spawnSync` does (`gh` and `git` here).
 *
 * Everything it downloads, `gh`'s own `gh-artifact.*.zip` among it, goes into one
 * `story-baselines-*` folder under `tmp`, which it removes when it finishes, taken or refused:
 * the only thing it keeps is the pictures it writes into `app/`. It first clears the folders that
 * killed pulls left there (`clearLeftovers`).
 */
export function pull({ pr, repo = REPO, root, tmp = tmpdir(), exec = spawnSync }) {
  clearLeftovers({ tmp });
  const run = (command, args, options = {}) => {
    const done = exec(command, args, { encoding: "utf8", ...options });
    if (done.status !== 0)
      throw new Refused(
        `${command} ${args.join(" ")} failed: ${(done.stderr || done.error?.message || "").trim()}`,
      );
    return done.stdout.trim();
  };
  const [head, from] = run("gh", [
    "pr",
    "view",
    pr,
    "--repo",
    repo,
    "--json",
    "headRefOid,headRepository,headRepositoryOwner",
    "--jq",
    '.headRefOid + " " + .headRepositoryOwner.login + "/" + .headRepository.name',
  ]).split(" ");
  const here = run("git", ["rev-parse", "HEAD"], { cwd: root });
  if (here !== head)
    throw new Refused(
      `this checkout is at ${here}, and pull request #${pr}'s head is ${head}: check out its head first, so the pictures are of the code they go in with`,
    );
  const runs = JSON.parse(
    run("gh", [
      "api",
      "-X",
      "GET",
      `repos/${repo}/actions/workflows/stories.yml/runs`,
      "-f",
      `head_sha=${head}`,
      "-f",
      "per_page=20",
      "--jq",
      ".workflow_runs",
    ]),
  );
  const chosen = runToTake(runs, head, from);
  if (!chosen.run) throw new Refused(chosen.why);
  const work = mkdtempSync(join(tmp, "story-baselines-"));
  try {
    const into = join(work, "artifact");
    // `gh run download` keeps the zip it fetches in the temp folder: this one, so it goes too.
    const scratch = join(work, "gh");
    mkdirSync(scratch);
    const got = exec(
      "gh",
      ["run", "download", String(chosen.run.id), "-R", repo, "-n", ARTIFACT, "-D", into],
      { encoding: "utf8", env: { ...process.env, TMPDIR: scratch, TMP: scratch, TEMP: scratch } },
    );
    if (got.status !== 0 || !existsSync(join(into, DRAWN)))
      throw new Refused(
        `the stories run ${chosen.run.html_url} kept no baselines: a story failed on more than its picture there, and its annotations say which`,
      );
    const drawn = JSON.parse(readFileSync(join(into, DRAWN), "utf8"));
    const wrong = notOf(drawn, head);
    if (wrong) throw new Refused(wrong);
    // The run tested the head merged with `base`: the same tree as the head only when the head
    // has every commit of it.
    const behind = run("gh", [
      "api",
      `repos/${repo}/compare/${drawn.base}...${head}`,
      "--jq",
      ".behind_by",
    ]);
    if (behind !== "0")
      throw new Refused(
        `the head is ${behind} commits behind the base its stories run merged it with, so the pictures are of another tree: update the branch, push, and take the next run's`,
      );
    // An artifact keeps no empty directory: with no baseline drawn, every one is deleted.
    mkdirSync(join(into, "app"), { recursive: true });
    const app = join(root, "app");
    const { taken, removed, unchanged } = take(join(into, "app"), app);
    return [
      ...removed.map((path) => `deleted: ${path}`),
      `${taken.length} baselines written and ${removed.length} deleted (${unchanged.length} already matched) into ${app}, from ${chosen.run.html_url}: ` +
        "review the pictures that changed (git status), then commit and push them",
    ];
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const given = process.argv.slice(2);
  const [pr] = given;
  const at = given.indexOf("--repo");
  const repo = at === -1 ? REPO : given[at + 1];
  if (!/^\d+$/.test(pr ?? "") || !/^[\w.-]+\/[\w.-]+$/.test(repo ?? "")) {
    console.error("usage: story-baselines-pull.mjs <pull request number> [--repo <owner>/<repo>]");
    process.exit(2);
  }
  const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  try {
    for (const line of pull({ pr, repo, root })) console.log(line);
  } catch (error) {
    // Set, not `process.exit`: an exit would skip the `finally` that removes the download.
    if (!(error instanceof Refused)) throw error;
    console.error(error.message);
    process.exitCode = 1;
  }
}
