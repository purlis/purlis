// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Which `stories` run a pull request's baselines are taken from, and whether its baselines are
// of the head (`tools/story-baselines-pull.mjs`). And that a pull, taken or refused, leaves nothing behind in the temp folder. Run with
// `node --test tools/`.
import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { crc32 } from "node:zlib";

import { DRAWN, notOf, pull, Refused, runToTake } from "./story-baselines-pull.mjs";

const HEAD = "a".repeat(40);
const BASE = "b".repeat(40);
const FROM = "purlis/purlis";

const runOf = (over) => ({
  id: 1,
  run_number: 1,
  run_attempt: 1,
  head_sha: HEAD,
  event: "pull_request",
  head_repository: { full_name: FROM },
  status: "completed",
  conclusion: "failure",
  html_url: "https://github.com/purlis/purlis/actions/runs/1",
  ...over,
});

test("the newest finished, failed run of the head is the one taken", () => {
  const newest = runOf({ id: 3, run_number: 3 });
  assert.equal(
    runToTake([runOf({ id: 2, run_number: 2 }), newest, runOf({ id: 9, run_number: 9, head_sha: BASE })], HEAD, FROM)
      .run,
    newest,
  );
  // A re-run of the same run is newer than its first attempt.
  const again = runOf({ id: 4, run_number: 4, run_attempt: 2 });
  assert.equal(runToTake([runOf({ id: 4, run_number: 4 }), again], HEAD, FROM).run, again);
});

test("only a pull_request run of the head's own repository is taken", () => {
  assert.match(
    runToTake([runOf({ event: "workflow_dispatch" })], HEAD, FROM).why,
    /no stories run has tested/,
  );
  assert.match(
    runToTake([runOf({ head_repository: { full_name: "someone/fork" } })], HEAD, FROM).why,
    /no stories run has tested/,
  );
  const mine = runOf({ id: 2, run_number: 2 });
  assert.equal(
    runToTake([mine, runOf({ id: 3, run_number: 3, event: "workflow_dispatch" })], HEAD, FROM).run,
    mine,
  );
});

test("no run, a run still going, or a run that passed is said, and nothing is taken", () => {
  assert.match(runToTake([], HEAD, FROM).why, /no stories run has tested/);
  assert.match(runToTake([runOf({ head_sha: BASE })], HEAD, FROM).why, /no stories run has tested/);
  assert.match(
    runToTake([runOf({ status: "in_progress", conclusion: null })], HEAD, FROM).why,
    /has not finished/,
  );
  assert.match(runToTake([runOf({ conclusion: "success" })], HEAD, FROM).why, /passed/);
  // An older failed run is not taken past a newer one that passed: the newest says what is so.
  assert.match(
    runToTake([runOf({ run_number: 1 }), runOf({ run_number: 2, conclusion: "success" })], HEAD, FROM)
      .why,
    /passed/,
  );
});

test("baselines drawn of another commit, or that name no base, are refused", () => {
  assert.equal(notOf({ head: HEAD, base: BASE }, HEAD), undefined);
  assert.match(notOf({ head: BASE, base: BASE }, HEAD), /drawn of/);
  assert.match(notOf({ head: HEAD }, HEAD), /which base/);
  assert.match(notOf({ head: HEAD, base: "main" }, HEAD), /which base/);
  assert.match(notOf(null, HEAD), /which commit/);
  assert.match(notOf([], HEAD), /which commit/);
});

// A pull against a stubbed `gh` and `git` (no network): what it leaves in the temp folder.
/** A whole 1×1 PNG, as the stories run draws them. */
const chunk = (type, data) => {
  const body = Buffer.concat([Buffer.from(type, "latin1"), data]);
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([length, body, crc]);
};
const IHDR = Buffer.from([0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0]);
const DRAWN_PNG = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk("IHDR", IHDR),
  chunk("IDAT", Buffer.from([0x78, 0x9c, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01])),
  chunk("IEND", Buffer.alloc(0)),
]);
const PICTURE = "src/ui/__screenshots__/button.stories.tsx/button_default_light-chromium-linux.png";

/** A `gh` and `git` that answer as GitHub would for pull request 7, whose stories run kept
 * baselines; `behind` is how far the head is behind the base the run merged it with. Its
 * download leaves a `gh-artifact.*.zip` in the temp folder it is given, as `gh` does. */
const githubOf = ({ tmp, behind = "0" }) => (command, args, options = {}) => {
  const said = (stdout) => ({ status: 0, stdout, stderr: "" });
  if (command === "git") return said(`${HEAD}\n`);
  if (args[0] === "pr") return said(`${HEAD} ${FROM}\n`);
  if (args[0] === "api" && args.includes("-X")) return said(JSON.stringify([runOf({})]));
  if (args[0] === "api") return said(`${behind}\n`);
  if (args[0] === "run") {
    writeFileSync(join(options.env?.TMPDIR ?? tmp, "gh-artifact.123.zip"), "zip");
    const into = args[args.indexOf("-D") + 1];
    mkdirSync(join(into, "app", dirname(PICTURE)), { recursive: true });
    writeFileSync(join(into, "app", PICTURE), DRAWN_PNG);
    writeFileSync(join(into, DRAWN), JSON.stringify({ head: HEAD, base: BASE }));
    return said("");
  }
  return { status: 1, stdout: "", stderr: `not stubbed: ${command} ${args.join(" ")}` };
};

const checkout = () => {
  const root = mkdtempSync(join(tmpdir(), "pull-root-"));
  mkdirSync(join(root, "app", "src"), { recursive: true });
  return root;
};

test("a pull that takes the baselines leaves nothing in the temp folder, and says where it wrote", (t) => {
  const tmp = mkdtempSync(join(tmpdir(), "pull-tmp-"));
  const root = checkout();
  t.after(() => [tmp, root].forEach((dir) => rmSync(dir, { recursive: true, force: true })));
  const said = pull({ pr: "7", repo: FROM, root, tmp, exec: githubOf({ tmp }) });
  assert.deepEqual(readFileSync(join(root, "app", PICTURE)), DRAWN_PNG);
  assert.match(said.join("\n"), new RegExp(`into ${join(root, "app")}`));
  assert.deepEqual(readdirSync(tmp), []);
});

test("a pull refused after the download leaves nothing in the temp folder, and writes nothing", (t) => {
  const tmp = mkdtempSync(join(tmpdir(), "pull-tmp-"));
  const root = checkout();
  t.after(() => [tmp, root].forEach((dir) => rmSync(dir, { recursive: true, force: true })));
  assert.throws(
    () => pull({ pr: "7", repo: FROM, root, tmp, exec: githubOf({ tmp, behind: "3" }) }),
    (error) => error instanceof Refused && /3 commits behind/.test(error.message),
  );
  assert.equal(existsSync(join(root, "app", PICTURE)), false);
  assert.deepEqual(readdirSync(tmp), []);
});
