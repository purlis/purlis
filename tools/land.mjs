#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **Land a pull request: the one way every session hands one to `main`'s merge queue** (#3501).
// The judgement around it (review, what goes in one pull request, a drop from the queue) is the
// `landing-a-pr` skill (`docs/agents/skills/landing-a-pr/SKILL.md`); this holds the mechanics.
//
//   node tools/land.mjs <pull request> [--sha <reviewed sha>] [--jump] [--watch] [--repo <owner>/<repo>]
//
// In order, it:
//   1. refuses unless the pull request is open and its base is `main`;
//   2. refuses when its head is not `--sha`, the commit the review approved, when one is given;
//   3. refuses while the newest `ci` run of main's head is red, unless the pull request carries
//      the `fixes-main` label, which is also enqueued at the front (`jump`), as `--jump` is;
//   4. reruns the runs of the head that wait on approval (`action_required`): gh counts those as
//      failing checks, so they are rerun before the checks are read;
//   5. reads the required checks: with `--watch` it waits for them to finish, and without it,
//      it refuses while any is pending; either way it refuses while any failed, naming each;
//   6. marks a draft ready (`gh pr ready`) and waits for the checks that starts;
//   7. reads main once more, then enqueues through GraphQL `enqueuePullRequest` with the head it
//      checked as `expectedHeadOid`, so a push since is refused by GitHub;
//   8. prints the queue position, and with `--watch` waits until the pull request merges, or
//      says it left the queue unmerged, with GitHub's reason and the failing checks of its queue
//      run, from their annotations.
//
// The queue tests each entry on top of main and the entries ahead of it, and merges on green, so
// this never updates the branch, never merges by hand, and never deletes a branch. It never
// downloads a log either: a check's annotations say why it failed.
//
// Exit status: 0 enqueued (merged, with `--watch`), 1 refused or failed, 2 usage, 3 dropped from
// the queue. It needs `gh`, signed in.
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

/** The repository a pull request is in, unless `--repo` names another. */
export const REPO = "purlis/purlis";

/** The label a pull request that turns a red main green carries: it lands past a red main. */
export const FIXES_MAIN = "fixes-main";

const USAGE =
  "usage: land.mjs <pull request number> [--sha <reviewed sha>] [--jump] [--watch] [--repo <owner>/<repo>]";

/** The options `argv` (after the script) asks for, or `{ usage }` when it is not a valid call. */
export function parseArgs(argv) {
  const options = { pr: undefined, sha: undefined, jump: false, watch: false, repo: REPO };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === "--jump") options.jump = true;
    else if (arg === "--watch") options.watch = true;
    else if (arg === "--sha") options.sha = argv[++i];
    else if (arg === "--repo") options.repo = argv[++i];
    else if (/^\d+$/.test(arg) && options.pr === undefined) options.pr = arg;
    else return { usage: `${USAGE}\nunexpected argument: ${arg}` };
  }
  if (options.pr === undefined) return { usage: USAGE };
  if (options.sha !== undefined && !/^[0-9a-f]{40}$/.test(options.sha))
    return { usage: `${USAGE}\n--sha takes the full 40-character commit the review approved` };
  if (!/^[\w.-]+\/[\w.-]+$/.test(options.repo ?? "")) return { usage: USAGE };
  return options;
}

/** Why the pull request `pr` (gh's `pr view` JSON) cannot land as it stands, or `undefined`. */
export function refusal(pr, sha) {
  if (pr.state !== "OPEN") return `pull request #${pr.number} is ${pr.state.toLowerCase()}`;
  if (pr.baseRefName !== "main")
    return `pull request #${pr.number} is based on ${pr.baseRefName}: only a pull request to main lands through its queue`;
  if (sha !== undefined && pr.headRefOid !== sha)
    return `pull request #${pr.number}'s head is ${pr.headRefOid}, not the reviewed ${sha}: review the new head first`;
  return undefined;
}

/** Whether the pull request carries the `fixes-main` label. */
export function fixesMain(pr) {
  return (pr.labels ?? []).some((label) => label.name === FIXES_MAIN);
}

const RED = new Set(["failure", "timed_out", "startup_failure"]);

/**
 * The red `ci` run of main's head, of `runs` (the `ci` workflow's runs of that commit), or
 * `undefined`: only the newest run counts, so a rerun that went green clears it, and a run still
 * going is not red.
 */
export function redRun(runs) {
  const newest = [...runs]
    .filter((run) => run.event === "push")
    .sort((a, b) => b.run_number - a.run_number || b.run_attempt - a.run_attempt)[0];
  return newest && newest.status === "completed" && RED.has(newest.conclusion) ? newest : undefined;
}

/**
 * The required checks (gh's `pr checks --required` JSON) sorted by gh's bucket: those still
 * going and those that failed. A check that passed or was skipped is in neither.
 */
export function sortChecks(checks) {
  const pending = checks.filter((check) => check.bucket === "pending");
  const failing = checks.filter((check) => check.bucket === "fail" || check.bucket === "cancel");
  return { pending, failing };
}

/** One line per check, for a refusal. */
const named = (checks) =>
  checks.map((check) => `  ${check.name}: ${check.state}${check.link ? ` ${check.link}` : ""}`);

/**
 * The runs of a merge-group commit to read when pull request `number` left the queue unmerged,
 * of `runs` (the repository's `merge_group` runs): those of the newest group it was tested in.
 */
export function queueRuns(runs, number) {
  const mine = runs.filter((run) =>
    (run.head_branch ?? "").startsWith(`gh-readonly-queue/main/pr-${number}-`),
  );
  const newest = [...mine].sort((a, b) => b.id - a.id)[0];
  return newest ? mine.filter((run) => run.head_sha === newest.head_sha) : [];
}

const seconds = (name, fallback) => {
  const value = Number(process.env[name]);
  return Number.isFinite(value) && process.env[name] !== "" ? value : fallback;
};
// How often a wait reads again, and how long the checks a `gh pr ready` starts take to appear.
// The tests set both to 0.
const POLL = seconds("LAND_POLL_SECONDS", 60);
const SETTLE = seconds("LAND_SETTLE_SECONDS", 20);
// How long a wait lasts before it gives up: the checks, then the queue.
const CHECKS_FOR = 120 * 60;
const QUEUE_FOR = 240 * 60;

const pause = (s) =>
  s > 0 && Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, s * 1000);

/** Stops with `message` and `status`. */
function stop(message, status = 1) {
  console.error(message);
  process.exit(status);
}

/** Runs gh and answers its result; `ok` is whether it exited 0. */
function ghRaw(args) {
  const done = spawnSync("gh", args, { encoding: "utf8" });
  return {
    ok: done.status === 0,
    stdout: (done.stdout ?? "").trim(),
    stderr: (done.stderr || done.error?.message || "").trim(),
  };
}

/** Runs gh and answers its standard output, parsed as JSON when `json`, or stops saying why. */
function gh(args, json = true) {
  const done = ghRaw(args);
  if (!done.ok) stop(`gh ${args.join(" ")} failed: ${done.stderr}`);
  return json ? JSON.parse(done.stdout || "null") : done.stdout;
}

function main(options) {
  const { repo } = options;
  const [owner, name] = repo.split("/");
  const viewPr = () =>
    gh([
      "pr",
      "view",
      options.pr,
      "--repo",
      repo,
      "--json",
      "number,id,state,baseRefName,headRefOid,isDraft,labels",
    ]);

  // 1, 2. The pull request itself.
  const pr = viewPr();
  const why = refusal(pr, options.sha);
  if (why) stop(why);
  const head = pr.headRefOid;
  const fixing = fixesMain(pr);
  const jump = options.jump || fixing;

  // 3. Main.
  const mainGate = () => {
    if (fixing) return;
    const mainHead = gh(["api", `repos/${repo}/commits/main`, "--jq", ".sha"], false);
    const runs = gh([
      "api",
      "-X",
      "GET",
      `repos/${repo}/actions/workflows/ci.yml/runs`,
      "-f",
      `head_sha=${mainHead}`,
      "-f",
      "per_page=20",
      "--jq",
      ".workflow_runs",
    ]);
    const red = redRun(runs ?? []);
    if (red)
      stop(
        `main is red: its ci run ${red.html_url} ended ${red.conclusion}. Nothing lands until main is green: ` +
          `land the pull request that fixes it, labelled ${FIXES_MAIN}, first`,
      );
  };
  mainGate();

  // 4. Runs waiting on approval.
  const waiting = gh([
    "api",
    "-X",
    "GET",
    `repos/${repo}/actions/runs`,
    "-f",
    `head_sha=${head}`,
    "-f",
    "status=action_required",
    "-f",
    "per_page=50",
    "--jq",
    ".workflow_runs",
  ]);
  for (const run of waiting ?? []) {
    gh(["run", "rerun", String(run.id), "--repo", repo], false);
    console.log(`reran ${run.name} (${run.html_url}), which waited on approval`);
  }

  // 5, 6. The required checks.
  const readChecks = () => {
    const done = ghRaw([
      "pr",
      "checks",
      options.pr,
      "--repo",
      repo,
      "--required",
      "--json",
      "name,state,bucket,link",
    ]);
    // gh exits non-zero while a check is pending or failed, and prints them all the same; with
    // none reported yet it prints nothing, which is a check still to come.
    if (done.stdout) return sortChecks(JSON.parse(done.stdout));
    if (/no (required )?checks reported/i.test(done.stderr))
      return {
        pending: [{ name: "every required check", state: "not reported yet" }],
        failing: [],
      };
    stop(`gh pr checks ${options.pr} failed: ${done.stderr}`);
  };
  const checksGate = (wait) => {
    let checks = readChecks();
    for (let waited = 0; wait && checks.pending.length > 0 && checks.failing.length === 0;) {
      if (waited >= CHECKS_FOR)
        stop(
          [
            `the required checks are still going after ${CHECKS_FOR / 60} minutes:`,
            ...named(checks.pending),
          ].join("\n"),
        );
      pause(POLL);
      waited += Math.max(POLL, 1);
      checks = readChecks();
    }
    if (checks.failing.length > 0)
      stop(
        [
          `pull request #${options.pr} has failing required checks; their annotations say why (gh api repos/${repo}/check-runs/<id>/annotations):`,
          ...named(checks.failing),
        ].join("\n"),
      );
    if (checks.pending.length > 0)
      stop(
        [
          `pull request #${options.pr} has required checks still going; run again when they finish, or with --watch:`,
          ...named(checks.pending),
        ].join("\n"),
      );
  };
  checksGate(options.watch);

  if (pr.isDraft) {
    gh(["pr", "ready", options.pr, "--repo", repo], false);
    console.log(`marked #${options.pr} ready for review; waiting for the checks that starts`);
    pause(SETTLE);
    checksGate(true);
  }

  // 7. Enqueue.
  mainGate();
  const entry = gh([
    "api",
    "graphql",
    "-f",
    "query=mutation($id: ID!, $head: GitObjectID!, $jump: Boolean) { enqueuePullRequest(input: {pullRequestId: $id, expectedHeadOid: $head, jump: $jump}) { mergeQueueEntry { state position } } }",
    "-f",
    `id=${pr.id}`,
    "-f",
    `head=${head}`,
    "-F",
    `jump=${jump}`,
    "--jq",
    ".data.enqueuePullRequest.mergeQueueEntry",
  ]);
  if (!entry) stop(`GitHub enqueued nothing for #${options.pr}`);

  // 8. Where it stands.
  console.log(
    `#${options.pr} enqueued at ${head}${jump ? " at the front" : ""}: position ${entry.position}, ${entry.state.toLowerCase()}`,
  );
  if (!options.watch) return;

  for (let waited = 0; ; waited += Math.max(POLL, 1)) {
    const now = gh([
      "api",
      "graphql",
      "-f",
      "query=query($owner: String!, $name: String!, $number: Int!) { repository(owner: $owner, name: $name) { pullRequest(number: $number) { state mergeQueueEntry { state position } timelineItems(itemTypes: [REMOVED_FROM_MERGE_QUEUE_EVENT], last: 1) { nodes { ... on RemovedFromMergeQueueEvent { reason } } } } } }",
      "-f",
      `owner=${owner}`,
      "-f",
      `name=${name}`,
      "-F",
      `number=${options.pr}`,
      "--jq",
      ".data.repository.pullRequest",
    ]);
    if (now.state === "MERGED") return console.log(`#${options.pr} merged`);
    if (now.state === "CLOSED") stop(`#${options.pr} was closed before it merged`);
    if (!now.mergeQueueEntry) return dropped(now.timelineItems?.nodes?.[0]?.reason);
    if (waited >= QUEUE_FOR)
      stop(`#${options.pr} is still in the queue after ${QUEUE_FOR / 60} minutes`);
    pause(POLL);
  }

  /** Says why the pull request left the queue unmerged, from the annotations of its queue run. */
  function dropped(reason) {
    const lines = [`#${options.pr} left the merge queue unmerged${reason ? `: ${reason}` : ""}`];
    const runs = gh([
      "api",
      "-X",
      "GET",
      `repos/${repo}/actions/runs`,
      "-f",
      "event=merge_group",
      "-f",
      "per_page=50",
      "--jq",
      ".workflow_runs",
    ]);
    for (const run of queueRuns(runs ?? [], options.pr)) {
      const jobs = gh([
        "api",
        "-X",
        "GET",
        `repos/${repo}/actions/runs/${run.id}/jobs`,
        "-f",
        "filter=latest",
        "-f",
        "per_page=100",
        "--jq",
        ".jobs",
      ]);
      for (const job of jobs ?? []) {
        if (!RED.has(job.conclusion)) continue;
        lines.push(`  ${run.name} / ${job.name}: ${job.conclusion} ${job.html_url}`);
        const notes = gh(["api", `repos/${repo}/check-runs/${job.id}/annotations`]) ?? [];
        for (const note of notes.filter((n) => n.annotation_level === "failure").slice(0, 5))
          lines.push(`    ${note.path}:${note.start_line}: ${note.message.split("\n")[0]}`);
      }
    }
    if (lines.length === 1)
      lines.push("  no queue run of it failed: GitHub's reason above is the cause");
    stop(lines.join("\n"), 3);
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const options = parseArgs(process.argv.slice(2));
  if (options.usage) stop(options.usage, 2);
  main(options);
}
