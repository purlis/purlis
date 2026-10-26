// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **A pull request's baselines, taken from its own `stories` run** (`docs/component-stories.md`):
// the pictures the pinned image drew of the stories, written into this checkout so they go into
// the next push like any other change.
//
//   node tools/story-baselines-pull.mjs <pull request>
//
// When a pull request's `stories` run fails on its pictures alone, the run draws them again,
// writing the baselines as they are now, and keeps them as `story-baselines`
// (`.github/workflows/stories.yml`). This takes that artifact from the newest `stories` run of
// the pull request's head and writes it into `app/` with `tools/story-baselines.mjs take`. The
// same check the `update-baselines` label's job does is done first: this checkout is the head
// that run tested, and that head has every commit of the base the run merged it with, so the
// pictures are of this tree. The person then reviews the pictures, commits them and pushes. A
// push of theirs starts every check as usual, where the label's commit, made with the
// workflow's own token, leaves them waiting for a maintainer.
//
// It needs `gh`, signed in, and `git`.
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { take } from "./story-baselines.mjs";

/** The artifact a `stories` run keeps its drawn baselines in, and the file that says of what. */
export const ARTIFACT = "story-baselines";
export const DRAWN = "drawn.json";

/**
 * The run whose baselines are taken, of `runs` (the `stories` workflow's runs of the head, as
 * the API lists them), or why there is none: the newest run, which must have finished and
 * failed (one that passed drew nothing; one that is still going has not drawn yet).
 */
export function runToTake(runs, head) {
  const newest = [...runs]
    .filter((run) => run.head_sha === head)
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

/** Runs `command` and answers its standard output, or exits saying what failed. */
function run(command, args, options = {}) {
  const done = spawnSync(command, args, { encoding: "utf8", ...options });
  if (done.status !== 0) {
    console.error(
      `${command} ${args.join(" ")} failed: ${(done.stderr || done.error?.message || "").trim()}`,
    );
    process.exit(1);
  }
  return done.stdout.trim();
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [pr] = process.argv.slice(2);
  if (!/^\d+$/.test(pr ?? "")) {
    console.error("usage: story-baselines-pull.mjs <pull request number>");
    process.exit(2);
  }
  const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const head = run("gh", ["pr", "view", pr, "--json", "headRefOid", "--jq", ".headRefOid"]);
  const here = run("git", ["rev-parse", "HEAD"], { cwd: root });
  if (here !== head) {
    console.error(
      `this checkout is at ${here}, and pull request #${pr}'s head is ${head}: check out its head first, so the pictures are of the code they go in with`,
    );
    process.exit(1);
  }
  const runs = JSON.parse(
    run("gh", [
      "api",
      "-X",
      "GET",
      "repos/{owner}/{repo}/actions/workflows/stories.yml/runs",
      "-f",
      `head_sha=${head}`,
      "-f",
      "per_page=20",
      "--jq",
      ".workflow_runs",
    ]),
  );
  const chosen = runToTake(runs, head);
  if (!chosen.run) {
    console.error(chosen.why);
    process.exit(1);
  }
  const into = mkdtempSync(join(tmpdir(), "story-baselines-"));
  try {
    const got = spawnSync(
      "gh",
      ["run", "download", String(chosen.run.id), "-n", ARTIFACT, "-D", into],
      { encoding: "utf8" },
    );
    if (got.status !== 0 || !existsSync(join(into, DRAWN))) {
      console.error(
        `the stories run ${chosen.run.html_url} kept no baselines: a story failed on more than its picture there, and its annotations say which`,
      );
      process.exit(1);
    }
    const drawn = JSON.parse(readFileSync(join(into, DRAWN), "utf8"));
    const wrong = notOf(drawn, head);
    if (wrong) {
      console.error(wrong);
      process.exit(1);
    }
    // The run tested the head merged with `base`: the same tree as the head only when the head
    // has every commit of it.
    const behind = run("gh", [
      "api",
      `repos/{owner}/{repo}/compare/${drawn.base}...${head}`,
      "--jq",
      ".behind_by",
    ]);
    if (behind !== "0") {
      console.error(
        `the head is ${behind} commits behind the base its stories run merged it with, so the pictures are of another tree: update the branch, push, and take the next run's`,
      );
      process.exit(1);
    }
    // An artifact keeps no empty directory: with no baseline drawn, every one is deleted.
    mkdirSync(join(into, "app"), { recursive: true });
    const { taken, removed } = take(join(into, "app"), join(root, "app"));
    for (const path of removed) console.log(`deleted: ${path}`);
    console.log(
      `${taken.length} baselines written and ${removed.length} deleted, from ${chosen.run.html_url}: ` +
        "review the pictures that changed (git status), then commit and push them",
    );
  } finally {
    rmSync(into, { recursive: true, force: true });
  }
}
