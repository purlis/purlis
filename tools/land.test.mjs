// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Landing a pull request (`tools/land.mjs`, #3501): its rules, and every refusal and the enqueue
// against a stand-in `gh`. Run with `node --test "tools/*.test.mjs"`.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdtempSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import {
  dropReason,
  fixesMain,
  parseArgs,
  queueRuns,
  redRun,
  refusal,
  requiredNames,
  sortChecks,
} from "./land.mjs";

const SCRIPT = join(dirname(fileURLToPath(import.meta.url)), "land.mjs");
const HEAD = "a".repeat(40);
const OTHER = "b".repeat(40);
const MAIN = "c".repeat(40);
const ENQUEUED_AT = "2026-10-27T12:00:00Z";

// A stand-in `gh`: each call is matched, in order, against the rules of a state file (a regular
// expression over its arguments joined by spaces); a rule answers its replies one call at a
// time, repeating its last. Every call is logged. A call no rule matches fails, so a test names
// every call the script makes.
//
// A reply is what the API answers, whole. When the call passes `--jq`, the stand-in applies it,
// as gh does, so the paths the script reads are tested too; it knows only plain paths
// (`.a.b`) and fails on anything else, rather than guess.
const FAKE_GH = `#!/usr/bin/env node
const fs = require("node:fs");
const args = process.argv.slice(2);
const line = args.join(" ");
fs.appendFileSync(process.env.FAKE_GH_LOG, JSON.stringify(args) + "\\n");
const state = JSON.parse(fs.readFileSync(process.env.FAKE_GH_STATE, "utf8"));
const rule = state.rules.find((r) => new RegExp(r.match).test(line));
if (!rule) { process.stderr.write("stand-in gh has no answer for: " + line); process.exit(1); }
const reply = rule.replies[Math.min(rule.used ?? 0, rule.replies.length - 1)];
rule.used = (rule.used ?? 0) + 1;
fs.writeFileSync(process.env.FAKE_GH_STATE, JSON.stringify(state));
let out = reply.out;
const jq = args.indexOf("--jq");
if (jq !== -1 && out !== undefined) {
  const path = args[jq + 1];
  if (!/^(\\.[A-Za-z_]+)+$/.test(path)) { process.stderr.write("stand-in gh cannot apply --jq " + path); process.exit(1); }
  for (const key of path.slice(1).split(".")) out = out === null || out === undefined ? null : (out[key] ?? null);
  out = typeof out === "string" ? out : JSON.stringify(out);
}
if (out !== undefined) process.stdout.write(typeof out === "string" ? out : JSON.stringify(out));
if (reply.err) process.stderr.write(reply.err);
process.exit(reply.exit ?? 0);
`;

const prOf = (over = {}) => ({
  number: 7,
  id: "PR_node7",
  state: "OPEN",
  baseRefName: "main",
  headRefOid: HEAD,
  isDraft: false,
  isCrossRepository: false,
  labels: [],
  ...over,
});
const ciRun = (over = {}) => ({
  id: 1,
  run_number: 1,
  run_attempt: 1,
  event: "push",
  status: "completed",
  conclusion: "success",
  html_url: "https://github.com/purlis/purlis/actions/runs/1",
  ...over,
});
const STATE = { pass: "SUCCESS", fail: "FAILURE", pending: "IN_PROGRESS", skipping: "SKIPPED" };
const check = (name, bucket, state = STATE[bucket] ?? bucket) => ({
  name,
  bucket,
  state,
  link: `https://ci/${name}`,
});
// The API's envelopes, which the script's `--jq` paths read into.
const runs = (list) => ({ total_count: list.length, workflow_runs: list });
const queued = (entry) => ({ data: { enqueuePullRequest: { mergeQueueEntry: entry } } });
const polled = (pr) => ({ data: { repository: { pullRequest: pr } } });

// Main's rules (the rulesets' rules for the branch) and its branch, with the checks they require.
const rulesFor = (contexts) =>
  contexts.length === 0
    ? []
    : [
        {
          type: "required_status_checks",
          ruleset_id: 1,
          parameters: {
            required_status_checks: contexts.map((context) => ({
              context,
              integration_id: 15368,
            })),
            strict_required_status_checks_policy: false,
          },
        },
      ];
const branchFor = (contexts) => ({
  name: "main",
  protected: contexts.length > 0,
  protection:
    contexts.length === 0
      ? {
          enabled: false,
          required_status_checks: { enforcement_level: "off", contexts: [] },
        }
      : {
          enabled: true,
          required_status_checks: { enforcement_level: "everyone", contexts },
        },
});

// The rules of a pull request that lands cleanly; a test overrides the ones it is about.
function rules(over = {}) {
  const all = {
    view: { match: "^pr view 7 ", replies: [{ out: prOf() }] },
    mainCi: { match: "workflows/ci.yml/runs", replies: [{ out: runs([ciRun()]) }] },
    waiting: { match: "status=action_required", replies: [{ out: runs([]) }] },
    rerun: { match: "^run rerun", replies: [{ out: "" }] },
    // By default main requires nothing by name, so `gh pr checks --required` alone decides.
    ruleset: {
      match: "^api repos/purlis/purlis/rules/branches/main",
      replies: [{ out: [] }],
    },
    branch: {
      match: "^api repos/purlis/purlis/branches/main",
      replies: [{ out: branchFor([]) }],
    },
    checks: {
      match: "^pr checks 7",
      replies: [{ out: [check("rust (fmt, clippy, test)", "pass"), check("stories", "skipping")] }],
    },
    ready: { match: "^pr ready 7", replies: [{ out: "" }] },
    enqueue: {
      match: "enqueuePullRequest",
      replies: [{ out: queued({ state: "QUEUED", position: 2, enqueuedAt: ENQUEUED_AT }) }],
    },
    poll: {
      match: "query=query",
      replies: [{ out: polled({ state: "MERGED", mergeQueueEntry: null }) }],
    },
    groups: { match: "event=merge_group", replies: [{ out: runs([]) }] },
    jobs: { match: "/jobs", replies: [{ out: { total_count: 0, jobs: [] } }] },
    notes: { match: "/annotations", replies: [{ out: [] }] },
    ...over,
  };
  return Object.values(all).filter(Boolean);
}

function land(args, over = {}, env = {}) {
  const dir = mkdtempSync(join(tmpdir(), "land-"));
  const gh = join(dir, "gh");
  writeFileSync(gh, FAKE_GH);
  chmodSync(gh, 0o755);
  const statePath = join(dir, "state.json");
  writeFileSync(statePath, JSON.stringify({ rules: rules(over) }));
  const logPath = join(dir, "calls.jsonl");
  const started = Date.now();
  const done = spawnSync(process.execPath, [SCRIPT, ...args], {
    encoding: "utf8",
    env: {
      ...process.env,
      PATH: `${dir}:${process.env.PATH}`,
      FAKE_GH_STATE: statePath,
      FAKE_GH_LOG: logPath,
      LAND_POLL_SECONDS: "0",
      LAND_SETTLE_SECONDS: "0",
      ...env,
    },
  });
  const calls = existsSync(logPath)
    ? readFileSync(logPath, "utf8").trim().split("\n").map(JSON.parse)
    : [];
  return {
    status: done.status,
    stdout: done.stdout,
    stderr: done.stderr,
    calls,
    took: Date.now() - started,
  };
}

const lines = (calls) => calls.map((args) => args.join(" "));
const enqueueCall = (calls) => calls.find((args) => args.join(" ").includes("enqueuePullRequest"));
const forbidden = (calls) =>
  lines(calls).filter((line) =>
    /update-branch|^pr merge|--delete-branch|run download|\/logs|\/approve/.test(line),
  );

// The rules.

test("the arguments name a pull request and the reviewed commit, which is required", () => {
  assert.deepEqual(parseArgs(["7", "--sha", HEAD]), {
    pr: "7",
    sha: HEAD,
    watch: false,
    repo: "purlis/purlis",
  });
  const all = parseArgs(["--watch", "7", "--repo", "a/b", "--sha", HEAD]);
  assert.deepEqual(all, { pr: "7", sha: HEAD, watch: true, repo: "a/b" });
  assert.match(parseArgs(["7"]).usage, /--sha names the commit the review approved/);
  assert.ok(parseArgs([]).usage);
  assert.ok(parseArgs(["7", "8", "--sha", HEAD]).usage);
  assert.match(parseArgs(["7", "--sha", "abc123"]).usage, /full 40-character/);
  assert.ok(parseArgs(["7", "--sha", HEAD, "--merge"]).usage);
  assert.ok(parseArgs(["7", "--sha", HEAD, "--repo", "nope"]).usage);
});

test("a flag missing its value is refused, not skipped", () => {
  assert.match(parseArgs(["7", "--watch", "--sha"]).usage, /--sha needs a value/);
  assert.match(parseArgs(["7", "--sha", "--watch"]).usage, /--sha needs a value/);
  assert.match(parseArgs(["7", "--sha", HEAD, "--repo"]).usage, /--repo needs a value/);
});

test("there is no --jump: only a fixes-main pull request goes to the front", () => {
  assert.match(parseArgs(["7", "--sha", HEAD, "--jump"]).usage, /unexpected argument: --jump/);
});

test("only an open pull request from this repository to main, at the reviewed head, may land", () => {
  assert.equal(refusal(prOf(), HEAD), undefined);
  assert.match(refusal(prOf({ baseRefName: "release" }), HEAD), /based on release/);
  assert.match(refusal(prOf(), OTHER), /not the reviewed/);
  assert.match(refusal(prOf({ state: "MERGED" }), HEAD), /is merged/);
  assert.match(refusal(prOf({ isCrossRepository: true }), HEAD), /from a fork/);
});

test("the fixes-main label is read off the pull request", () => {
  assert.equal(fixesMain(prOf()), false);
  assert.equal(fixesMain(prOf({ labels: [{ name: "fixes-main" }] })), true);
});

test("main is red when its newest finished push run ended red", () => {
  assert.equal(redRun([]), undefined);
  assert.equal(redRun([ciRun()]), undefined);
  assert.ok(redRun([ciRun({ conclusion: "failure" })]));
  assert.ok(redRun([ciRun({ conclusion: "timed_out" })]));
  // A rerun that went green clears it.
  assert.equal(
    redRun([ciRun({ conclusion: "failure" }), ciRun({ run_attempt: 2, conclusion: "success" })]),
    undefined,
  );
  // A newer run still going says nothing yet: the red one before it still counts.
  assert.ok(
    redRun([
      ciRun({ conclusion: "failure" }),
      ciRun({ run_number: 2, status: "in_progress", conclusion: null }),
    ]),
  );
  // A merge-queue or nightly run is not main's run.
  assert.equal(redRun([ciRun({ event: "schedule", conclusion: "failure" })]), undefined);
});

test("checks sort into still going and failed, by gh's bucket", () => {
  const sorted = sortChecks([
    check("a", "pass"),
    check("b", "pending"),
    check("c", "fail"),
    check("d", "skipping"),
    check("e", "cancel", "CANCELLED"),
  ]);
  assert.deepEqual(
    sorted.pending.map((c) => c.name),
    ["b"],
  );
  assert.deepEqual(
    sorted.failing.map((c) => c.name),
    ["c", "e"],
  );
});

test("a required check that has not posted is still going", () => {
  const sorted = sortChecks(
    [check("rust", "pass"), check("web", "fail")],
    ["rust", "web", "scenario tests (ubuntu-24.04)"],
  );
  assert.deepEqual(sorted.pending, [
    { name: "scenario tests (ubuntu-24.04)", state: "not posted yet" },
  ]);
  assert.deepEqual(
    sorted.failing.map((c) => c.name),
    ["web"],
  );
  // With no names known, what gh lists is all there is.
  assert.deepEqual(sortChecks([check("rust", "pass")]).pending, []);
});

test("the required names are those of main's rulesets and of its branch protection, together", () => {
  assert.deepEqual(
    requiredNames(rulesFor(["rust", "docs"]), branchFor(["docs", "web"]).protection),
    ["rust", "docs", "web"],
  );
  assert.deepEqual(requiredNames([], branchFor([]).protection), []);
  assert.deepEqual(requiredNames([{ type: "merge_queue", parameters: {} }], undefined), []);
});

test("a drop's reason is only one given since this enqueue", () => {
  const at = (createdAt, reason) => ({ createdAt, reason });
  assert.equal(dropReason([at("2026-10-27T11:00:00Z", "an earlier drop")], ENQUEUED_AT), undefined);
  assert.equal(
    dropReason(
      [at("2026-10-27T11:00:00Z", "an earlier drop"), at("2026-10-27T12:30:00Z", "checks failed")],
      ENQUEUED_AT,
    ),
    "checks failed",
  );
  assert.equal(dropReason(undefined, ENQUEUED_AT), undefined);
});

test("a drop is read from the newest merge group the pull request was tested in", () => {
  const run = (id, sha, branch = `gh-readonly-queue/main/pr-7-${sha}`) => ({
    id,
    head_sha: sha,
    head_branch: branch,
  });
  const picked = queueRuns(
    [run(1, OTHER), run(5, HEAD), run(6, HEAD), run(9, MAIN, "gh-readonly-queue/main/pr-70-x")],
    "7",
  );
  assert.deepEqual(
    picked.map((r) => r.id),
    [5, 6],
  );
  assert.deepEqual(queueRuns([], "7"), []);
});

// Against a stand-in gh.

test("a usage error says how to call it, and calls nothing", () => {
  for (const args of [[], ["7"], ["7", "--sha"], ["7", "--sha", HEAD, "--jump"]]) {
    const got = land(args);
    assert.equal(got.status, 2, args.join(" "));
    assert.match(got.stderr, /usage: land\.mjs/);
    assert.deepEqual(got.calls, []);
  }
});

test("a pull request to another base is refused", () => {
  const got = land(["7", "--sha", HEAD], {
    view: { match: "^pr view 7 ", replies: [{ out: prOf({ baseRefName: "next" }) }] },
  });
  assert.equal(got.status, 1);
  assert.match(got.stderr, /based on next/);
  assert.equal(enqueueCall(got.calls), undefined);
});

test("a closed pull request is refused", () => {
  const got = land(["7", "--sha", HEAD], {
    view: { match: "^pr view 7 ", replies: [{ out: prOf({ state: "CLOSED" }) }] },
  });
  assert.equal(got.status, 1);
  assert.match(got.stderr, /is closed/);
});

test("a fork's pull request is refused before any of its runs is approved or rerun", () => {
  const got = land(["7", "--sha", HEAD], {
    view: { match: "^pr view 7 ", replies: [{ out: prOf({ isCrossRepository: true }) }] },
    waiting: {
      match: "status=action_required",
      replies: [{ out: runs([{ id: 41, name: "ci", html_url: "https://ci/41" }]) }],
    },
  });
  assert.equal(got.status, 1);
  assert.match(got.stderr, /from a fork/);
  assert.deepEqual(lines(got.calls), [
    "pr view 7 --repo purlis/purlis --json number,id,state,baseRefName,headRefOid,isDraft,isCrossRepository,labels",
  ]);
});

test("a head other than the reviewed sha is refused", () => {
  const got = land(["7", "--sha", OTHER]);
  assert.equal(got.status, 1);
  assert.match(got.stderr, new RegExp(`${HEAD}, not the reviewed ${OTHER}`));
  assert.equal(enqueueCall(got.calls), undefined);
});

test("a red main is refused, naming its run, read from the newest finished push run", () => {
  const got = land(["7", "--sha", HEAD], {
    mainCi: {
      match: "workflows/ci.yml/runs",
      replies: [{ out: runs([ciRun({ conclusion: "failure" })]) }],
    },
  });
  assert.equal(got.status, 1);
  assert.match(
    got.stderr,
    /main is red: its ci run https:\/\/github.com\/purlis\/purlis\/actions\/runs\/1 ended failure/,
  );
  assert.match(got.stderr, /fixes-main/);
  assert.equal(enqueueCall(got.calls), undefined);
  // Main's colour is asked of its finished push runs, not of its head's run, which may still be
  // going while the one before it is red.
  const asked = got.calls.find((args) => args.join(" ").includes("workflows/ci.yml/runs"));
  for (const query of ["branch=main", "event=push", "status=completed", "per_page=1"])
    assert.ok(asked.includes(query), query);
});

test("main going red while the checks are waited for is refused before the enqueue", () => {
  const got = land(["7", "--sha", HEAD, "--watch"], {
    mainCi: {
      match: "workflows/ci.yml/runs",
      replies: [{ out: runs([ciRun()]) }, { out: runs([ciRun({ conclusion: "failure" })]) }],
    },
  });
  assert.equal(got.status, 1);
  assert.match(got.stderr, /main is red/);
  assert.equal(enqueueCall(got.calls), undefined);
});

test("a fixes-main pull request lands past a red main, at the front of the queue", () => {
  const got = land(["7", "--sha", HEAD], {
    view: {
      match: "^pr view 7 ",
      replies: [{ out: prOf({ labels: [{ name: "fixes-main" }] }) }],
    },
    mainCi: {
      match: "workflows/ci.yml/runs",
      replies: [{ out: runs([ciRun({ conclusion: "failure" })]) }],
    },
  });
  assert.equal(got.status, 0, got.stderr);
  assert.ok(enqueueCall(got.calls).includes("jump=true"));
  assert.match(got.stdout, /at the front/);
});

test("a failing required check is refused, naming it, whatever gh's exit status", () => {
  for (const exit of [0, 1]) {
    const got = land(["7", "--sha", HEAD, "--watch"], {
      checks: {
        match: "^pr checks 7",
        replies: [{ out: [check("rust (fmt, clippy, test)", "fail"), check("web", "pass")], exit }],
      },
    });
    assert.equal(got.status, 1);
    assert.match(got.stderr, /failing required checks/);
    assert.match(got.stderr, /rust \(fmt, clippy, test\): FAILURE https:\/\/ci\/rust/);
    assert.doesNotMatch(got.stderr, /\bweb:/);
    assert.equal(enqueueCall(got.calls), undefined);
  }
});

test("a pending required check is refused without --watch, naming it", () => {
  const got = land(["7", "--sha", HEAD], {
    checks: { match: "^pr checks 7", replies: [{ out: [check("stories", "pending")], exit: 8 }] },
  });
  assert.equal(got.status, 1);
  assert.match(got.stderr, /still going; run again when they finish, or with --watch/);
  assert.match(got.stderr, /stories: IN_PROGRESS/);
  assert.equal(enqueueCall(got.calls), undefined);
});

test("no required check reported yet counts as pending", () => {
  const got = land(["7", "--sha", HEAD], {
    checks: {
      match: "^pr checks 7",
      replies: [{ err: "no required checks reported on the 'x' branch", exit: 1 }],
    },
  });
  assert.equal(got.status, 1);
  assert.match(got.stderr, /not reported yet/);
});

test("with --watch, pending checks are waited for, then it lands", () => {
  const got = land(["7", "--sha", HEAD, "--watch"], {
    checks: {
      match: "^pr checks 7",
      replies: [
        { out: [check("stories", "pending")], exit: 8 },
        { out: [check("stories", "pending")], exit: 8 },
        { out: [check("stories", "pass")] },
      ],
    },
  });
  assert.equal(got.status, 0, got.stderr);
  assert.equal(got.calls.filter((args) => args[0] === "pr" && args[1] === "checks").length, 3);
  assert.match(got.stdout, /#7 merged/);
});

test("runs waiting on approval are rerun, then given a moment, before the checks are read", () => {
  const waiting = {
    match: "status=action_required",
    replies: [{ out: runs([{ id: 41, name: "ci", html_url: "https://ci/41" }]) }],
  };
  const got = land(["7", "--sha", HEAD], { waiting }, { LAND_SETTLE_SECONDS: "1" });
  assert.equal(got.status, 0, got.stderr);
  const said = lines(got.calls);
  const rerun = said.indexOf("run rerun 41 --repo purlis/purlis");
  assert.ok(rerun >= 0);
  assert.ok(rerun < said.findIndex((line) => line.startsWith("pr checks")));
  assert.ok(
    said.some((line) => line.includes(`head_sha=${HEAD}`) && line.includes("action_required")),
  );
  // The pause after a rerun is what keeps the old, failed check from being read.
  assert.ok(got.took >= 1000, `took ${got.took}ms`);
  // With nothing rerun, there is no pause.
  assert.ok(land(["7", "--sha", HEAD], {}, { LAND_SETTLE_SECONDS: "2" }).took < 2000);
});

test("a required check that has never posted is refused without --watch, naming it", () => {
  const required = ["rust (fmt, clippy, test)", "scenario tests (ubuntu-24.04)"];
  const got = land(["7", "--sha", HEAD], {
    branch: {
      match: "^api repos/purlis/purlis/branches/main",
      replies: [{ out: branchFor(required) }],
    },
    checks: {
      match: "^pr checks 7",
      replies: [{ out: [check("rust (fmt, clippy, test)", "pass")] }],
    },
  });
  assert.equal(got.status, 1);
  assert.match(got.stderr, /still going; run again when they finish, or with --watch/);
  assert.match(got.stderr, /scenario tests \(ubuntu-24\.04\): not posted yet/);
  assert.doesNotMatch(got.stderr, /rust \(fmt/);
  assert.equal(enqueueCall(got.calls), undefined);
});

test("with none reported yet, each required check is named as not posted", () => {
  const got = land(["7", "--sha", HEAD], {
    ruleset: {
      match: "^api repos/purlis/purlis/rules/branches/main",
      replies: [{ out: rulesFor(["docs (build, links, reachable)"]) }],
    },
    checks: {
      match: "^pr checks 7",
      replies: [{ err: "no required checks reported on the 'x' branch", exit: 1 }],
    },
  });
  assert.equal(got.status, 1);
  assert.match(got.stderr, /docs \(build, links, reachable\): not posted yet/);
});

test("with --watch, after a rerun it waits until every required check has posted and passed", () => {
  // #3539: the rerun runs' checks had not posted when the checks were read, so the tool read
  // nothing pending and enqueued, and GitHub refused: "5 of 9 required status checks are expected".
  const required = ["rust (fmt, clippy, test)", "stories", "docs (build, links, reachable)"];
  const got = land(["7", "--sha", HEAD, "--watch"], {
    waiting: {
      match: "status=action_required",
      replies: [{ out: runs([{ id: 41, name: "docs", html_url: "https://ci/41" }]) }],
    },
    ruleset: {
      match: "^api repos/purlis/purlis/rules/branches/main",
      replies: [{ out: rulesFor(["stories"]) }],
    },
    branch: {
      match: "^api repos/purlis/purlis/branches/main",
      replies: [{ out: branchFor(required) }],
    },
    checks: {
      match: "^pr checks 7",
      replies: [
        {
          out: [check("rust (fmt, clippy, test)", "pass"), check("stories", "pass")],
        },
        {
          out: [check("rust (fmt, clippy, test)", "pass"), check("stories", "pass")],
        },
        {
          out: [
            check("rust (fmt, clippy, test)", "pass"),
            check("stories", "pass"),
            check("docs (build, links, reachable)", "pending"),
          ],
          exit: 8,
        },
        {
          out: [
            check("rust (fmt, clippy, test)", "pass"),
            check("stories", "pass"),
            check("docs (build, links, reachable)", "pass"),
          ],
        },
      ],
    },
  });
  assert.equal(got.status, 0, got.stderr);
  const said = lines(got.calls);
  const checkReads = said
    .map((line, i) => (line.startsWith("pr checks") ? i : -1))
    .filter((i) => i >= 0);
  assert.equal(checkReads.length, 4);
  assert.ok(said.indexOf("run rerun 41 --repo purlis/purlis") < checkReads[0]);
  assert.ok(checkReads[3] < said.findIndex((line) => line.includes("enqueuePullRequest")));
  assert.match(got.stdout, /#7 merged/);
});

test("with --watch, a required check that never posts is given up on, named, and not enqueued", () => {
  const got = land(
    ["7", "--sha", HEAD, "--watch"],
    {
      branch: {
        match: "^api repos/purlis/purlis/branches/main",
        replies: [{ out: branchFor(["recorded behaviour"]) }],
      },
    },
    { LAND_CHECKS_FOR_SECONDS: "3" },
  );
  assert.equal(got.status, 1);
  assert.match(got.stderr, /still going after 3 seconds/);
  assert.match(got.stderr, /recorded behaviour: not posted yet/);
  assert.equal(enqueueCall(got.calls), undefined);
});

test("a draft is marked ready, and its checks read again, before the enqueue", () => {
  const got = land(["7", "--sha", HEAD], {
    view: { match: "^pr view 7 ", replies: [{ out: prOf({ isDraft: true }) }] },
    checks: {
      match: "^pr checks 7",
      replies: [
        { out: [check("rust", "pass")] },
        { out: [check("rust", "pass"), check("e2e", "pending")], exit: 8 },
        { out: [check("rust", "pass"), check("e2e", "pass")] },
      ],
    },
  });
  assert.equal(got.status, 0, got.stderr);
  const said = lines(got.calls);
  const ready = said.indexOf("pr ready 7 --repo purlis/purlis");
  assert.ok(ready >= 0);
  assert.equal(said.filter((line, i) => i > ready && line.startsWith("pr checks")).length, 2);
  assert.ok(ready < said.findIndex((line) => line.includes("enqueuePullRequest")));
});

test("a check failing after the ready is refused", () => {
  const got = land(["7", "--sha", HEAD], {
    view: { match: "^pr view 7 ", replies: [{ out: prOf({ isDraft: true }) }] },
    checks: {
      match: "^pr checks 7",
      replies: [{ out: [check("rust", "pass")] }, { out: [check("e2e", "fail")], exit: 1 }],
    },
  });
  assert.equal(got.status, 1);
  assert.match(got.stderr, /e2e: FAILURE/);
  assert.equal(enqueueCall(got.calls), undefined);
});

test("the enqueue names the pull request, the reviewed head and no jump, and prints the position", () => {
  const got = land(["7", "--sha", HEAD]);
  assert.equal(got.status, 0, got.stderr);
  const call = enqueueCall(got.calls);
  assert.deepEqual(call.slice(0, 2), ["api", "graphql"]);
  assert.ok(call.includes("id=PR_node7"));
  assert.ok(call.includes(`head=${HEAD}`));
  assert.ok(call.includes("jump=false"));
  const query = call.find((arg) => arg.startsWith("query="));
  assert.match(
    query,
    /enqueuePullRequest\(input: \{pullRequestId: \$id, expectedHeadOid: \$head, jump: \$jump\}\)/,
  );
  assert.match(got.stdout, new RegExp(`#7 enqueued at ${HEAD}: position 2, queued`));
  // Without --watch it stops at the enqueue.
  assert.ok(!lines(got.calls).some((line) => line.includes("query=query")));
  assert.deepEqual(forbidden(got.calls), []);
  // An open, non-draft pull request is not marked ready again.
  assert.ok(!got.calls.some((args) => args[0] === "pr" && args[1] === "ready"));
});

test("an enqueue GitHub answers with no entry is refused", () => {
  const got = land(["7", "--sha", HEAD], {
    enqueue: { match: "enqueuePullRequest", replies: [{ out: queued(null) }] },
  });
  assert.equal(got.status, 1);
  assert.match(got.stderr, /GitHub enqueued nothing for #7/);
});

test("with --watch it waits through the queue until the pull request merges", () => {
  const got = land(["7", "--sha", HEAD, "--watch"], {
    poll: {
      match: "query=query",
      replies: [
        {
          out: polled({
            state: "OPEN",
            mergeQueueEntry: { state: "AWAITING_CHECKS", position: 1 },
          }),
        },
        { out: polled({ state: "MERGED", mergeQueueEntry: null }) },
      ],
    },
  });
  assert.equal(got.status, 0, got.stderr);
  assert.match(got.stdout, /#7 merged/);
  assert.deepEqual(forbidden(got.calls), []);
});

test("an entry gone a moment before the merge shows is a merge, not a drop", () => {
  const got = land(["7", "--sha", HEAD, "--watch"], {
    poll: {
      match: "query=query",
      replies: [
        { out: polled({ state: "OPEN", mergeQueueEntry: null, timelineItems: { nodes: [] } }) },
        { out: polled({ state: "MERGED", mergeQueueEntry: null }) },
      ],
    },
  });
  assert.equal(got.status, 0, got.stderr);
  assert.match(got.stdout, /#7 merged/);
  assert.ok(!lines(got.calls).some((line) => line.includes("event=merge_group")));
});

test("a drop from the queue names GitHub's reason and the failing check, from its annotations", () => {
  const branch = `gh-readonly-queue/main/pr-7-${MAIN}`;
  const gone = polled({
    state: "OPEN",
    mergeQueueEntry: null,
    timelineItems: {
      nodes: [
        { createdAt: "2026-10-27T09:00:00Z", reason: "an earlier drop" },
        { createdAt: "2026-10-27T12:40:00Z", reason: "the required checks failed" },
      ],
    },
  });
  const got = land(["7", "--sha", HEAD, "--watch"], {
    poll: { match: "query=query", replies: [{ out: gone }] },
    groups: {
      match: "event=merge_group",
      replies: [{ out: runs([{ id: 90, name: "ci", head_sha: OTHER, head_branch: branch }]) }],
    },
    jobs: {
      match: "runs/90/jobs",
      replies: [
        {
          out: {
            total_count: 2,
            jobs: [
              { id: 901, name: "rust (1)", conclusion: "failure", html_url: "https://ci/901" },
              { id: 902, name: "web", conclusion: "success", html_url: "https://ci/902" },
            ],
          },
        },
      ],
    },
    notes: {
      match: "check-runs/901/annotations",
      replies: [
        {
          out: [
            {
              annotation_level: "failure",
              path: "crates/x.rs",
              start_line: 12,
              message: "assertion failed\nmore",
            },
            { annotation_level: "warning", path: "y", start_line: 1, message: "noise" },
          ],
        },
      ],
    },
  });
  assert.equal(got.status, 3);
  assert.match(got.stderr, /left the merge queue unmerged: the required checks failed/);
  assert.doesNotMatch(got.stderr, /an earlier drop/);
  assert.match(got.stderr, /ci \/ rust \(1\): failure https:\/\/ci\/901/);
  assert.match(got.stderr, /crates\/x\.rs:12: assertion failed$/m);
  assert.doesNotMatch(got.stderr, /noise|web/);
  assert.ok(!lines(got.calls).some((line) => line.includes("check-runs/902")));
  // A drop is said only after a second read still finds the entry gone.
  assert.equal(lines(got.calls).filter((line) => line.includes("query=query")).length, 2);
  assert.deepEqual(forbidden(got.calls), []);
});

test("a drop with no failed queue run says GitHub's reason is the cause", () => {
  const got = land(["7", "--sha", HEAD, "--watch"], {
    poll: {
      match: "query=query",
      replies: [
        {
          out: polled({
            state: "OPEN",
            mergeQueueEntry: null,
            timelineItems: {
              nodes: [{ createdAt: "2026-10-27T12:01:00Z", reason: "merge conflict" }],
            },
          }),
        },
      ],
    },
  });
  assert.equal(got.status, 3);
  assert.match(got.stderr, /unmerged: merge conflict/);
  assert.match(got.stderr, /no queue run of it failed/);
});

test("a pull request closed while queued is reported", () => {
  const got = land(["7", "--sha", HEAD, "--watch"], {
    poll: {
      match: "query=query",
      replies: [{ out: polled({ state: "CLOSED", mergeQueueEntry: null }) }],
    },
  });
  assert.equal(got.status, 1);
  assert.match(got.stderr, /closed before it merged/);
});

test("a failed gh call stops it, saying which", () => {
  const got = land(["7", "--sha", HEAD], {
    enqueue: { match: "enqueuePullRequest", replies: [{ err: "HTTP 422: head moved", exit: 1 }] },
  });
  assert.equal(got.status, 1);
  assert.match(got.stderr, /failed: HTTP 422: head moved/);
});
