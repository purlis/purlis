// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Which `stories` run a pull request's baselines are taken from, and whether its baselines are
// of the head (`tools/story-baselines-pull.mjs`). And that a pull, taken or refused, leaves nothing
// behind in the temp folder, and clears what a killed one left. Run with
// `node --test tools/`.
import assert from "node:assert/strict";
import {
  existsSync,
  lutimesSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  realpathSync,
  rmSync,
  symlinkSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { crc32 } from "node:zlib";

import { clearLeftovers, DRAWN, notOf, pull, Refused, runToTake } from "./story-baselines-pull.mjs";

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

// A killed pull never reaches its `finally`, so the next pull clears what one left (#5348).
const DAY = 24 * 60 * 60 * 1000;
const NOW = Date.parse("2026-11-07T12:00:00Z");

/** A folder at `path` holding `files`, every one of them, and the folders, last written at `at`. */
const leftAt = (path, at, files = ["artifact/app/picture.png", "gh/gh-artifact.1.zip"]) => {
  for (const file of files) {
    mkdirSync(dirname(join(path, file)), { recursive: true });
    writeFileSync(join(path, file), "x");
  }
  const when = new Date(at);
  const touch = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      if (entry.isDirectory()) touch(join(dir, entry.name));
      else utimesSync(join(dir, entry.name), when, when);
    }
    utimesSync(dir, when, when);
  };
  touch(path);
};

test("a pull's folder nothing has written to for a day is removed, and one written to since is kept", (t) => {
  const tmp = mkdtempSync(join(tmpdir(), "pull-tmp-"));
  t.after(() => rmSync(tmp, { recursive: true, force: true }));
  leftAt(join(tmp, "story-baselines-kill01"), NOW - DAY - 1000);
  leftAt(join(tmp, "story-baselines-alive1"), NOW - 60_000);
  // Old folders, but a pull alongside is still writing one file in it.
  leftAt(join(tmp, "story-baselines-alive2"), NOW - 3 * DAY);
  writeFileSync(join(tmp, "story-baselines-alive2", "artifact", "app", "picture.png"), "y");
  utimesSync(join(tmp, "story-baselines-alive2", "artifact", "app", "picture.png"), new Date(NOW), new Date(NOW));

  const removed = clearLeftovers({ tmp, now: NOW });

  assert.deepEqual(readdirSync(tmp).sort(), ["story-baselines-alive1", "story-baselines-alive2"]);
  assert.deepEqual(removed, [join(realpathSync(tmp), "story-baselines-kill01")]);
});

test("only a folder whose name is a pull's own is removed, never a file, a link or what it points at", (t) => {
  const tmp = mkdtempSync(join(tmpdir(), "pull-tmp-"));
  const elsewhere = mkdtempSync(join(tmpdir(), "pull-elsewhere-"));
  t.after(() => [tmp, elsewhere].forEach((dir) => rmSync(dir, { recursive: true, force: true })));
  const old = NOW - 3 * DAY;
  for (const name of ["story-baselines-", "story-baselines-abc", "story-baselines-abcdef.bak", "x-story-baselines-abcdef", "story-baselines-abc/ef"])
    leftAt(join(tmp, name), old);
  leftAt(elsewhere, old);
  symlinkSync(elsewhere, join(tmp, "story-baselines-link01"));
  writeFileSync(join(tmp, "story-baselines-file01"), "x");
  utimesSync(join(tmp, "story-baselines-file01"), new Date(old), new Date(old));
  const before = readdirSync(tmp).sort();

  assert.deepEqual(clearLeftovers({ tmp, now: NOW }), []);

  assert.deepEqual(readdirSync(tmp).sort(), before);
  assert.equal(existsSync(join(elsewhere, "artifact", "app", "picture.png")), true);
});

test("a folder nothing has written to for a day is removed even when it holds a link, and the link's target is kept", (t) => {
  const tmp = mkdtempSync(join(tmpdir(), "pull-tmp-"));
  const elsewhere = mkdtempSync(join(tmpdir(), "pull-elsewhere-"));
  t.after(() => [tmp, elsewhere].forEach((dir) => rmSync(dir, { recursive: true, force: true })));
  writeFileSync(join(elsewhere, "keep"), "x");
  leftAt(join(tmp, "story-baselines-kill02"), NOW - 2 * DAY);
  symlinkSync(elsewhere, join(tmp, "story-baselines-kill02", "artifact", "out"));
  const when = new Date(NOW - 2 * DAY);
  lutimesSync(join(tmp, "story-baselines-kill02", "artifact", "out"), when, when);
  utimesSync(join(tmp, "story-baselines-kill02", "artifact"), when, when);

  assert.equal(clearLeftovers({ tmp, now: NOW }).length, 1);

  assert.deepEqual(readdirSync(tmp), []);
  assert.equal(existsSync(join(elsewhere, "keep")), true);
});

test("a temp folder that resolves to nothing is never cleared", () => {
  assert.deepEqual(clearLeftovers({ tmp: "", now: NOW }), []);
  assert.deepEqual(clearLeftovers({ tmp: join(tmpdir(), "no-such-folder-5348"), now: NOW }), []);
});

test("a pull clears a killed pull's folder from the temp folder before it downloads", (t) => {
  const tmp = mkdtempSync(join(tmpdir(), "pull-tmp-"));
  const root = checkout();
  t.after(() => [tmp, root].forEach((dir) => rmSync(dir, { recursive: true, force: true })));
  leftAt(join(tmp, "story-baselines-kill03"), Date.now() - 2 * DAY);
  leftAt(join(tmp, "story-baselines-alive3"), Date.now());

  pull({ pr: "7", repo: FROM, root, tmp, exec: githubOf({ tmp }) });

  assert.deepEqual(readdirSync(tmp), ["story-baselines-alive3"]);
});
