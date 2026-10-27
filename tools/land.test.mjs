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

import { fixesMain, parseArgs, queueRuns, redRun, refusal, sortChecks } from "./land.mjs";

const SCRIPT = join(dirname(fileURLToPath(import.meta.url)), "land.mjs");
const HEAD = "a".repeat(40);
const OTHER = "b".repeat(40);
const MAIN = "c".repeat(40);

// A stand-in `gh`: each call is matched, in order, against the rules of a state file (a regular
// expression over its arguments joined by spaces); a rule answers its replies one call at a
// time, repeating its last. Every call is logged. A call no rule matches fails, so a test names
// every call the script makes.
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
if (reply.out !== undefined) process.stdout.write(typeof reply.out === "string" ? reply.out : JSON.stringify(reply.out));
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
const check = (name, bucket, state = bucket) => ({
  name,
  bucket,
  state,
  link: `https://ci/${name}`,
});

// The rules of a pull request that lands cleanly; a test overrides the ones it is about.
function rules(over = {}) {
  const all = {
    view: { match: "^pr view 7 ", replies: [{ out: prOf() }] },
    mainHead: { match: "commits/main", replies: [{ out: MAIN }] },
    mainCi: { match: "workflows/ci.yml/runs", replies: [{ out: [ciRun()] }] },
    waiting: { match: "status=action_required", replies: [{ out: [] }] },
    rerun: { match: "^run rerun", replies: [{ out: "" }] },
    checks: {
      match: "^pr checks 7",
      replies: [{ out: [check("rust (fmt, clippy, test)", "pass"), check("stories", "skipping")] }],
    },
    ready: { match: "^pr ready 7", replies: [{ out: "" }] },
    enqueue: {
      match: "enqueuePullRequest",
      replies: [{ out: { state: "QUEUED", position: 2 } }],
    },
    poll: { match: "query=query", replies: [{ out: { state: "MERGED", mergeQueueEntry: null } }] },
    groups: { match: "event=merge_group", replies: [{ out: [] }] },
    jobs: { match: "/jobs", replies: [{ out: [] }] },
    notes: { match: "/annotations", replies: [{ out: [] }] },
    ...over,
  };
  return Object.values(all).filter(Boolean);
}

function land(args, over = {}) {
  const dir = mkdtempSync(join(tmpdir(), "land-"));
  const gh = join(dir, "gh");
  writeFileSync(gh, FAKE_GH);
  chmodSync(gh, 0o755);
  const statePath = join(dir, "state.json");
  writeFileSync(statePath, JSON.stringify({ rules: rules(over) }));
  const logPath = join(dir, "calls.jsonl");
  const done = spawnSync(process.execPath, [SCRIPT, ...args], {
    encoding: "utf8",
    env: {
      ...process.env,
      PATH: `${dir}:${process.env.PATH}`,
      FAKE_GH_STATE: statePath,
      FAKE_GH_LOG: logPath,
      LAND_POLL_SECONDS: "0",
      LAND_SETTLE_SECONDS: "0",
    },
  });
  const calls = existsSync(logPath)
    ? readFileSync(logPath, "utf8").trim().split("\n").map(JSON.parse)
    : [];
  return { status: done.status, stdout: done.stdout, stderr: done.stderr, calls };
}

const enqueueCall = (calls) => calls.find((args) => args.join(" ").includes("enqueuePullRequest"));
const forbidden = (calls) =>
  calls.filter((args) => {
    const line = args.join(" ");
    return /update-branch|^pr merge|--delete-branch|run download|\/logs/.test(line);
  });

// The rules.

test("the arguments name a pull request, and --sha takes a full commit", () => {
  assert.deepEqual(parseArgs(["7"]), {
    pr: "7",
    sha: undefined,
    jump: false,
    watch: false,
    repo: "purlis/purlis",
  });
  const all = parseArgs(["7", "--sha", HEAD, "--jump", "--watch", "--repo", "a/b"]);
  assert.deepEqual(all, { pr: "7", sha: HEAD, jump: true, watch: true, repo: "a/b" });
  assert.ok(parseArgs([]).usage);
  assert.ok(parseArgs(["7", "8"]).usage);
  assert.ok(parseArgs(["7", "--sha", "abc123"]).usage);
  assert.ok(parseArgs(["7", "--merge"]).usage);
  assert.ok(parseArgs(["7", "--repo", "nope"]).usage);
});

test("only an open pull request to main, at the reviewed head, may land", () => {
  assert.equal(refusal(prOf(), undefined), undefined);
  assert.equal(refusal(prOf(), HEAD), undefined);
  assert.match(refusal(prOf({ baseRefName: "release" }), undefined), /based on release/);
  assert.match(refusal(prOf(), OTHER), /not the reviewed/);
  assert.match(refusal(prOf({ state: "MERGED" }), undefined), /is merged/);
});

test("the fixes-main label is read off the pull request", () => {
  assert.equal(fixesMain(prOf()), false);
  assert.equal(fixesMain(prOf({ labels: [{ name: "fixes-main" }] })), true);
});

test("main is red only when the newest push run of its head ended red", () => {
  assert.equal(redRun([]), undefined);
  assert.equal(redRun([ciRun()]), undefined);
  assert.ok(redRun([ciRun({ conclusion: "failure" })]));
  assert.ok(redRun([ciRun({ conclusion: "timed_out" })]));
  // A rerun that went green clears it; one still going is not red.
  assert.equal(
    redRun([ciRun({ conclusion: "failure" }), ciRun({ run_attempt: 2, conclusion: "success" })]),
    undefined,
  );
  assert.equal(
    redRun([
      ciRun({ conclusion: "failure" }),
      ciRun({ run_number: 2, status: "in_progress", conclusion: null }),
    ]),
    undefined,
  );
  // A merge-queue or nightly run of the same commit is not main's run.
  assert.equal(redRun([ciRun({ event: "schedule", conclusion: "failure" })]), undefined);
});

test("checks sort into still going and failed, by gh's bucket", () => {
  const sorted = sortChecks([
    check("a", "pass"),
    check("b", "pending"),
    check("c", "fail"),
    check("d", "skipping"),
    check("e", "cancel"),
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

test("a usage error says how to call it", () => {
  const got = land([]);
  assert.equal(got.status, 2);
  assert.match(got.stderr, /usage: land\.mjs/);
  assert.deepEqual(got.calls, []);
});

test("a pull request to another base is refused", () => {
  const got = land(["7"], {
    view: { match: "^pr view 7 ", replies: [{ out: prOf({ baseRefName: "next" }) }] },
  });
  assert.equal(got.status, 1);
  assert.match(got.stderr, /based on next/);
  assert.equal(enqueueCall(got.calls), undefined);
});

test("a closed pull request is refused", () => {
  const got = land(["7"], {
    view: { match: "^pr view 7 ", replies: [{ out: prOf({ state: "CLOSED" }) }] },
  });
  assert.equal(got.status, 1);
  assert.match(got.stderr, /is closed/);
});

test("a head other than the reviewed sha is refused", () => {
  const got = land(["7", "--sha", OTHER]);
  assert.equal(got.status, 1);
  assert.match(got.stderr, new RegExp(`${HEAD}, not the reviewed ${OTHER}`));
  assert.equal(enqueueCall(got.calls), undefined);
});

test("a red main is refused, naming its run", () => {
  const got = land(["7"], {
    mainCi: {
      match: "workflows/ci.yml/runs",
      replies: [{ out: [ciRun({ conclusion: "failure" })] }],
    },
  });
  assert.equal(got.status, 1);
  assert.match(
    got.stderr,
    /main is red: its ci run https:\/\/github.com\/purlis\/purlis\/actions\/runs\/1 ended failure/,
  );
  assert.match(got.stderr, /fixes-main/);
  assert.equal(enqueueCall(got.calls), undefined);
  // main's run is read for main's head.
  assert.ok(got.calls.some((args) => args.includes(`head_sha=${MAIN}`)));
});

test("main going red while the checks are waited for is refused before the enqueue", () => {
  const got = land(["7", "--watch"], {
    mainCi: {
      match: "workflows/ci.yml/runs",
      replies: [{ out: [ciRun()] }, { out: [ciRun({ conclusion: "failure" })] }],
    },
  });
  assert.equal(got.status, 1);
  assert.match(got.stderr, /main is red/);
  assert.equal(enqueueCall(got.calls), undefined);
});

test("a fixes-main pull request lands past a red main, at the front of the queue", () => {
  const got = land(["7"], {
    view: { match: "^pr view 7 ", replies: [{ out: prOf({ labels: [{ name: "fixes-main" }] }) }] },
    mainCi: {
      match: "workflows/ci.yml/runs",
      replies: [{ out: [ciRun({ conclusion: "failure" })] }],
    },
  });
  assert.equal(got.status, 0, got.stderr);
  assert.ok(enqueueCall(got.calls).includes("jump=true"));
  assert.match(got.stdout, /at the front/);
});

test("--jump enqueues at the front", () => {
  const got = land(["7", "--jump"]);
  assert.equal(got.status, 0, got.stderr);
  assert.ok(enqueueCall(got.calls).includes("jump=true"));
});

test("a failing required check is refused, naming it", () => {
  const got = land(["7", "--watch"], {
    checks: {
      match: "^pr checks 7",
      replies: [
        {
          out: [check("rust (fmt, clippy, test)", "fail", "FAILURE"), check("web", "pass")],
          exit: 1,
        },
      ],
    },
  });
  assert.equal(got.status, 1);
  assert.match(got.stderr, /failing required checks/);
  assert.match(got.stderr, /rust \(fmt, clippy, test\): FAILURE https:\/\/ci\/rust/);
  assert.doesNotMatch(got.stderr, /\bweb:/);
  assert.equal(enqueueCall(got.calls), undefined);
});

test("a pending required check is refused without --watch, naming it", () => {
  const got = land(["7"], {
    checks: {
      match: "^pr checks 7",
      replies: [{ out: [check("stories", "pending", "IN_PROGRESS")], exit: 8 }],
    },
  });
  assert.equal(got.status, 1);
  assert.match(got.stderr, /still going; run again when they finish, or with --watch/);
  assert.match(got.stderr, /stories: IN_PROGRESS/);
  assert.equal(enqueueCall(got.calls), undefined);
});

test("no required check reported yet counts as pending", () => {
  const got = land(["7"], {
    checks: {
      match: "^pr checks 7",
      replies: [{ err: "no required checks reported on the 'x' branch", exit: 1 }],
    },
  });
  assert.equal(got.status, 1);
  assert.match(got.stderr, /not reported yet/);
});

test("with --watch, pending checks are waited for, then it lands", () => {
  const got = land(["7", "--watch"], {
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

test("runs waiting on approval are rerun before the checks are read", () => {
  const got = land(["7"], {
    waiting: {
      match: "status=action_required",
      replies: [{ out: [{ id: 41, name: "ci", html_url: "https://ci/41" }] }],
    },
  });
  assert.equal(got.status, 0, got.stderr);
  const lines = got.calls.map((args) => args.join(" "));
  const rerun = lines.findIndex((line) => line === "run rerun 41 --repo purlis/purlis");
  assert.ok(rerun >= 0);
  assert.ok(rerun < lines.findIndex((line) => line.startsWith("pr checks")));
  assert.ok(
    lines.some((line) => line.includes(`head_sha=${HEAD}`) && line.includes("action_required")),
  );
});

test("a draft is marked ready, and the checks that starts are waited for before the enqueue", () => {
  const got = land(["7"], {
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
  const lines = got.calls.map((args) => args.join(" "));
  const ready = lines.indexOf("pr ready 7 --repo purlis/purlis");
  assert.ok(ready >= 0);
  assert.equal(lines.filter((line, i) => i > ready && line.startsWith("pr checks")).length, 2);
  assert.ok(ready < lines.findIndex((line) => line.includes("enqueuePullRequest")));
});

test("a check the ready starts that fails is refused", () => {
  const got = land(["7"], {
    view: { match: "^pr view 7 ", replies: [{ out: prOf({ isDraft: true }) }] },
    checks: {
      match: "^pr checks 7",
      replies: [
        { out: [check("rust", "pass")] },
        { out: [check("e2e", "fail", "FAILURE")], exit: 1 },
      ],
    },
  });
  assert.equal(got.status, 1);
  assert.match(got.stderr, /e2e: FAILURE/);
  assert.equal(enqueueCall(got.calls), undefined);
});

test("the enqueue names the pull request, the checked head and no jump, and prints the position", () => {
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
  assert.ok(!got.calls.some((args) => args.join(" ").includes("query=query")));
  assert.deepEqual(forbidden(got.calls), []);
  // An open, non-draft pull request is not marked ready again.
  assert.ok(!got.calls.some((args) => args[0] === "pr" && args[1] === "ready"));
});

test("with --watch it waits through the queue until the pull request merges", () => {
  const got = land(["7", "--watch"], {
    poll: {
      match: "query=query",
      replies: [
        { out: { state: "OPEN", mergeQueueEntry: { state: "AWAITING_CHECKS", position: 1 } } },
        { out: { state: "MERGED", mergeQueueEntry: null } },
      ],
    },
  });
  assert.equal(got.status, 0, got.stderr);
  assert.match(got.stdout, /#7 merged/);
  assert.deepEqual(forbidden(got.calls), []);
});

test("a drop from the queue names GitHub's reason and the failing check, from its annotations", () => {
  const branch = `gh-readonly-queue/main/pr-7-${MAIN}`;
  const got = land(["7", "--watch"], {
    poll: {
      match: "query=query",
      replies: [
        {
          out: {
            state: "OPEN",
            mergeQueueEntry: null,
            timelineItems: { nodes: [{ reason: "the required checks failed" }] },
          },
        },
      ],
    },
    groups: {
      match: "event=merge_group",
      replies: [{ out: [{ id: 90, name: "ci", head_sha: OTHER, head_branch: branch }] }],
    },
    jobs: {
      match: "runs/90/jobs",
      replies: [
        {
          out: [
            { id: 901, name: "rust (1)", conclusion: "failure", html_url: "https://ci/901" },
            { id: 902, name: "web", conclusion: "success", html_url: "https://ci/902" },
          ],
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
  assert.match(got.stderr, /ci \/ rust \(1\): failure https:\/\/ci\/901/);
  assert.match(got.stderr, /crates\/x\.rs:12: assertion failed$/m);
  assert.doesNotMatch(got.stderr, /noise|web/);
  assert.ok(!got.calls.some((args) => args.join(" ").includes("check-runs/902")));
  assert.deepEqual(forbidden(got.calls), []);
});

test("a drop with no failed queue run says GitHub's reason is the cause", () => {
  const got = land(["7", "--watch"], {
    poll: {
      match: "query=query",
      replies: [
        {
          out: {
            state: "OPEN",
            mergeQueueEntry: null,
            timelineItems: { nodes: [{ reason: "merge conflict" }] },
          },
        },
      ],
    },
  });
  assert.equal(got.status, 3);
  assert.match(got.stderr, /unmerged: merge conflict/);
  assert.match(got.stderr, /no queue run of it failed/);
});

test("a pull request closed while queued is reported", () => {
  const got = land(["7", "--watch"], {
    poll: { match: "query=query", replies: [{ out: { state: "CLOSED", mergeQueueEntry: null } }] },
  });
  assert.equal(got.status, 1);
  assert.match(got.stderr, /closed before it merged/);
});

test("a failed gh call stops it, saying which", () => {
  const got = land(["7"], {
    enqueue: { match: "enqueuePullRequest", replies: [{ err: "HTTP 422: head moved", exit: 1 }] },
  });
  assert.equal(got.status, 1);
  assert.match(got.stderr, /failed: HTTP 422: head moved/);
});
