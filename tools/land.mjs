#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **Land a pull request: the one way every session hands one to `main`'s merge queue** (#3501).
// The judgement around it (review, what goes in one pull request, a drop from the queue) is the
// `landing-a-pr` skill (`docs/agents/skills/landing-a-pr/SKILL.md`); this holds the mechanics.
//
//   node tools/land.mjs <pull request> --sha <reviewed sha> [--watch] [--repo <owner>/<repo>]
//
// In order, it:
//   1. refuses unless the pull request is open, from a branch of this repository (approving a
//      fork's runs is a person's call), and based on `main`;
//   2. refuses when its head is not `--sha`, the commit the review approved;
//   3. refuses while main is red: the newest finished `ci` run of a push to main failed. A pull
//      request labelled `fixes-main` lands past a red main, and only it goes to the front of the
//      queue (`jump`), since a jump restarts every group being built;
//   4. reruns the runs of the head that wait on approval (`action_required`): gh counts those as
//      failing checks, so they are rerun, and given a moment to start, before the checks are read;
//   5. reads the required checks: their names from main's rulesets and branch protection, and
//      their states from `gh pr checks --required`, which lists only the checks that have posted,
//      so a required check not posted yet (a rerun's, just started) counts as pending. With
//      `--watch` it waits until every one has posted and finished, and without it, it refuses
//      while any is pending; either way it refuses while any failed, naming each. A wait that
//      can only end in GitHub's refusal stops early: when the only checks left are required ones
//      that have not posted, and no run of the head is still going to post them, for a few polls
//      running (a job renamed while protection still names it, or a run waiting on approval);
//   6. marks a draft ready (`gh pr ready`), which the queue needs, and reads the checks again. No
//      workflow here starts on `ready_for_review` today; the second read is a safety net for one
//      that does;
//   7. reads main once more, then enqueues through GraphQL `enqueuePullRequest` with the head it
//      checked as `expectedHeadOid`, so a push since is refused by GitHub;
//   8. prints the queue position, and with `--watch` waits until the pull request merges, or
//      says it left the queue unmerged, with GitHub's reason and the failing checks of the queue
//      run this enqueue started, from their annotations. A merge conflict built no run: it says
//      so, and prints no checks.
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
  "usage: land.mjs <pull request number> --sha <reviewed sha> [--watch] [--repo <owner>/<repo>]";

/** The options `argv` (after the script) asks for, or `{ usage }` when it is not a valid call. */
export function parseArgs(argv) {
  const options = { pr: undefined, sha: undefined, watch: false, repo: REPO };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === "--watch") options.watch = true;
    else if (arg === "--sha" || arg === "--repo") {
      const value = argv[++i];
      if (value === undefined || value.startsWith("--"))
        return { usage: `${USAGE}\n${arg} needs a value` };
      options[arg.slice(2)] = value;
    } else if (/^\d+$/.test(arg) && options.pr === undefined) options.pr = arg;
    else return { usage: `${USAGE}\nunexpected argument: ${arg}` };
  }
  if (options.pr === undefined) return { usage: USAGE };
  if (options.sha === undefined)
    return { usage: `${USAGE}\n--sha names the commit the review approved, and is required` };
  if (!/^[0-9a-f]{40}$/.test(options.sha))
    return { usage: `${USAGE}\n--sha takes the full 40-character commit the review approved` };
  if (!/^[\w.-]+\/[\w.-]+$/.test(options.repo)) return { usage: USAGE };
  return options;
}

/** Why the pull request `pr` (gh's `pr view` JSON) cannot land as it stands, or `undefined`. */
export function refusal(pr, sha) {
  if (pr.state !== "OPEN") return `pull request #${pr.number} is ${pr.state.toLowerCase()}`;
  if (pr.isCrossRepository)
    return `pull request #${pr.number} is from a fork: a maintainer reviews and approves its runs, and lands it, by hand`;
  if (pr.baseRefName !== "main")
    return `pull request #${pr.number} is based on ${pr.baseRefName}: only a pull request to main lands through its queue`;
  if (pr.headRefOid !== sha)
    return `pull request #${pr.number}'s head is ${pr.headRefOid}, not the reviewed ${sha}: review the new head first`;
  return undefined;
}

/** Whether the pull request carries the `fixes-main` label. */
export function fixesMain(pr) {
  return (pr.labels ?? []).some((label) => label.name === FIXES_MAIN);
}

const RED = new Set(["failure", "timed_out", "startup_failure"]);

/**
 * The run that makes main red, of `runs` (the `ci` workflow's runs of pushes to main), or
 * `undefined`. Main's colour is its newest *finished* push run: a run still going says nothing
 * yet, so a red main stays red until a run goes green, however busy the queue is. A rerun that
 * went green clears it, and a nightly or merge-queue run is not main's.
 */
export function redRun(runs) {
  const newest = [...runs]
    .filter((run) => run.event === "push" && run.status === "completed")
    .sort((a, b) => b.run_number - a.run_number || b.run_attempt - a.run_attempt)[0];
  return newest && RED.has(newest.conclusion) ? newest : undefined;
}

/**
 * The names of the checks main requires, from `rules` (the rules GitHub applies to the branch,
 * from its rulesets) and from `protection` (the branch's classic protection, as the branch reads
 * it): either can require a check, and the queue waits on both.
 */
export function requiredNames(rules, protection) {
  const fromRules = (rules ?? [])
    .filter((rule) => rule.type === "required_status_checks")
    .flatMap((rule) => rule.parameters?.required_status_checks ?? [])
    .map((check) => check.context);
  const fromProtection = protection?.required_status_checks?.contexts ?? [];
  return [...new Set([...fromRules, ...fromProtection])];
}

/**
 * The required checks (gh's `pr checks --required` JSON) sorted by gh's bucket: those still
 * going and those that failed. A check that passed or was skipped is in neither. gh lists only
 * the checks that have posted, so each of `required` (main's required names) it does not list
 * is still going too: its run has not started, or has only just been rerun.
 */
export function sortChecks(checks, required = []) {
  const posted = new Set(checks.map((check) => check.name));
  const pending = [
    ...checks.filter((check) => check.bucket === "pending"),
    ...required
      .filter((name) => !posted.has(name))
      .map((name) => ({ name, state: "not posted yet" })),
  ];
  const failing = checks.filter((check) => check.bucket === "fail" || check.bucket === "cancel");
  return { pending, failing };
}

/** Whether a workflow run (the REST API's) is still going, and so may yet post a check. */
const going = (run) => run.status !== "completed" && run.status !== "action_required";

/**
 * The required names that will not post, of `checks` (`sortChecks`'s answer) and `runs` (the
 * workflow runs of the head): those not posted yet, when every check that has posted has finished
 * and no run of the head is still going. Anything still going might post them, so then it is none.
 */
export function unposted(checks, runs) {
  const missing = checks.pending.filter((check) => check.state === "not posted yet");
  if (missing.length === 0 || missing.length < checks.pending.length) return [];
  if ((runs ?? []).some(going)) return [];
  return missing.map((check) => check.name);
}

/** Whether GitHub's `reason` for a drop is a merge conflict, which builds no queue run. */
export function conflicted(reason) {
  return /merge[\s_-]*conflict/i.test(reason ?? "");
}

/** One line per check, for a refusal. */
const named = (checks) =>
  checks.map((check) => `  ${check.name}: ${check.state}${check.link ? ` ${check.link}` : ""}`);

/**
 * GitHub's reason the pull request last left the queue, of `events` (its
 * `RemovedFromMergeQueueEvent`s), counting only one at or after `since` (when this enqueue was
 * made): an older one is from an earlier time in the queue.
 */
export function dropReason(events, since) {
  const fresh = (events ?? []).filter((event) => event && event.createdAt >= since);
  return fresh.sort((a, b) => b.createdAt.localeCompare(a.createdAt))[0]?.reason;
}

/**
 * The runs of a merge-group commit to read when pull request `number` left the queue unmerged,
 * of `runs` (the repository's `merge_group` runs): those of the newest group it was tested in
 * since `since` (when this enqueue was made). A group from an earlier time in the queue is not
 * why it left this time.
 */
export function queueRuns(runs, number, since = "") {
  const mine = runs.filter(
    (run) =>
      (run.head_branch ?? "").startsWith(`gh-readonly-queue/main/pr-${number}-`) &&
      (run.created_at ?? "") >= since,
  );
  const newest = [...mine].sort((a, b) => b.id - a.id)[0];
  return newest ? mine.filter((run) => run.head_sha === newest.head_sha) : [];
}

const seconds = (name, fallback) => {
  const value = Number(process.env[name]);
  return Number.isFinite(value) && process.env[name] !== "" ? value : fallback;
};
// How often a wait reads again, and how long a change GitHub has just been told of takes to show:
// a rerun's checks to start, a ready pull request's, or a merge to land after the queue lets the
// entry go. The tests set both to 0.
const POLL = seconds("LAND_POLL_SECONDS", 60);
const SETTLE = seconds("LAND_SETTLE_SECONDS", 30);
// How long a wait lasts before it gives up: the checks, then the queue. The tests shorten the first.
const CHECKS_FOR = seconds("LAND_CHECKS_FOR_SECONDS", 120 * 60);
const QUEUE_FOR = 240 * 60;
// How many polls running the required checks left must be ones that have not posted, with no run
// of the head going, before the wait gives up on them: a run just queued takes a moment to show.
const UNPOSTED_POLLS = 3;
const waitedFor = (s) => (s % 60 === 0 ? `${s / 60} minutes` : `${s} seconds`);

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
      "number,id,state,baseRefName,headRefOid,isDraft,isCrossRepository,labels",
    ]);

  // 1, 2. The pull request itself.
  const pr = viewPr();
  const why = refusal(pr, options.sha);
  if (why) stop(why);
  const head = pr.headRefOid;
  const fixing = fixesMain(pr);

  // 3. Main.
  const mainGate = () => {
    if (fixing) return;
    const runs = gh([
      "api",
      "-X",
      "GET",
      `repos/${repo}/actions/workflows/ci.yml/runs`,
      "-f",
      "branch=main",
      "-f",
      "event=push",
      "-f",
      "status=completed",
      "-f",
      "per_page=1",
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
  // Until the rerun starts, gh still reads its old check as failed.
  if ((waiting ?? []).length > 0) pause(SETTLE);

  // 5, 6. The required checks. Their names come from main's rules, since gh lists only the
  // checks that have posted: right after a rerun, a required check can be missing altogether.
  // The rules come a page at a time, 30 to a page: a required check can be on any page.
  const required = requiredNames(
    (gh(["api", `repos/${repo}/rules/branches/main`, "--paginate", "--slurp"]) ?? []).flat(),
    gh(["api", `repos/${repo}/branches/main`, "--jq", ".protection"]),
  );
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
    // The checks are read from what gh prints, whatever its exit status: with --json, gh 2.83
    // exits 0 even with a check failed, and older ones exit 1 or 8 while one is failed or
    // pending. With none reported yet it prints nothing, which is a check still to come.
    if (done.stdout) return sortChecks(JSON.parse(done.stdout), required);
    if (/no (required )?checks reported/i.test(done.stderr))
      return required.length > 0
        ? sortChecks([], required)
        : {
            pending: [{ name: "every required check", state: "not reported yet" }],
            failing: [],
          };
    stop(`gh pr checks ${options.pr} failed: ${done.stderr}`);
  };
  const headRuns = () =>
    gh([
      "api",
      "-X",
      "GET",
      `repos/${repo}/actions/runs`,
      "-f",
      `head_sha=${head}`,
      "-f",
      "per_page=100",
      "--jq",
      ".workflow_runs",
    ]);
  const checksGate = (wait) => {
    let checks = readChecks();
    let stuck = { names: "", polls: 0 };
    let said = "";
    for (let waited = 0; wait && checks.pending.length > 0 && checks.failing.length === 0;) {
      if (waited >= CHECKS_FOR)
        stop(
          [
            `the required checks are still going after ${waitedFor(CHECKS_FOR)}:`,
            ...named(checks.pending),
          ].join("\n"),
        );
      // Each change in what has not posted is said, so a long wait names what it waits on.
      const missing = checks.pending.filter((check) => check.state === "not posted yet");
      const now = missing.map((check) => check.name).join(", ");
      if (now && now !== said) console.log(`waiting on required checks not posted yet: ${now}`);
      said = now;
      // The runs are read only when nothing posted is still going: it is the one case they decide.
      const names = missing.length === checks.pending.length ? unposted(checks, headRuns()) : [];
      const key = names.join("\n");
      stuck = {
        names: key,
        polls: names.length === 0 ? 0 : key === stuck.names ? stuck.polls + 1 : 1,
      };
      if (stuck.polls >= UNPOSTED_POLLS)
        stop(
          [
            `pull request #${options.pr} waits on required checks that have not posted, and no run of its head is going to post them:`,
            ...names.map((name) => `  ${name}`),
            "A job renamed or removed while main still requires its old name, or a run waiting on approval, does this; GitHub would refuse the enqueue. Fix main's required checks or the run, and land again.",
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
    console.log(`marked #${options.pr} ready for review; reading its checks again`);
    pause(SETTLE);
    checksGate(true);
  }

  // 7. Enqueue.
  mainGate();
  const entry = gh([
    "api",
    "graphql",
    "-f",
    "query=mutation($id: ID!, $head: GitObjectID!, $jump: Boolean) { enqueuePullRequest(input: {pullRequestId: $id, expectedHeadOid: $head, jump: $jump}) { mergeQueueEntry { state position enqueuedAt } } }",
    "-f",
    `id=${pr.id}`,
    "-f",
    `head=${head}`,
    "-F",
    `jump=${fixing}`,
    "--jq",
    ".data.enqueuePullRequest.mergeQueueEntry",
  ]);
  if (!entry) stop(`GitHub enqueued nothing for #${options.pr}`);

  // 8. Where it stands.
  console.log(
    `#${options.pr} enqueued at ${head}${fixing ? " at the front" : ""}: position ${entry.position}, ${entry.state.toLowerCase()}`,
  );
  if (!options.watch) return;

  const read = () =>
    gh([
      "api",
      "graphql",
      "-f",
      "query=query($owner: String!, $name: String!, $number: Int!) { repository(owner: $owner, name: $name) { pullRequest(number: $number) { state mergeQueueEntry { state position } timelineItems(itemTypes: [REMOVED_FROM_MERGE_QUEUE_EVENT], last: 5) { nodes { ... on RemovedFromMergeQueueEvent { reason createdAt } } } } } }",
      "-f",
      `owner=${owner}`,
      "-f",
      `name=${name}`,
      "-F",
      `number=${options.pr}`,
      "--jq",
      ".data.repository.pullRequest",
    ]);
  for (let waited = 0; ; waited += Math.max(POLL, 1)) {
    let now = read();
    // While a merge lands, the entry can be gone a moment before the pull request reads MERGED:
    // only an entry still gone after a pause is a drop.
    if (now.state === "OPEN" && !now.mergeQueueEntry) {
      pause(SETTLE);
      now = read();
    }
    if (now.state === "MERGED") return console.log(`#${options.pr} merged`);
    if (now.state === "CLOSED") stop(`#${options.pr} was closed before it merged`);
    if (!now.mergeQueueEntry)
      return dropped(dropReason(now.timelineItems?.nodes, entry.enqueuedAt ?? ""));
    if (waited >= QUEUE_FOR)
      stop(`#${options.pr} is still in the queue after ${QUEUE_FOR / 60} minutes`);
    pause(POLL);
  }

  /** Says why the pull request left the queue unmerged, from the annotations of its queue run. */
  function dropped(reason) {
    const lines = [`#${options.pr} left the merge queue unmerged${reason ? `: ${reason}` : ""}`];
    // A conflict is found before any group is built, so no queue run says anything about it, and
    // an older group's failures would only mislead.
    if (conflicted(reason)) {
      lines.push(
        "  it conflicts with main, or with an entry ahead of it: bring main into its branch, resolve the conflict, have the new head reviewed, and land again",
      );
      stop(lines.join("\n"), 3);
    }
    // The newest 50 merge-group runs of the whole repository: with at most five groups building
    // and a few workflows each, that holds the group this pull request just left, read within a
    // poll of it leaving. Keep it at 50 or more.
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
    for (const run of queueRuns(runs ?? [], options.pr, entry.enqueuedAt ?? "")) {
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
