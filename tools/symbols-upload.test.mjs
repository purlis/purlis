// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// What `release.yml`'s `symbols` job does with the debug files a build saved (#5692): run as the
// job runs it, with a stand-in `sentry-cli` that records what it was asked and can be told to
// fail. Run with `node --test tools/`.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const TOOL = fileURLToPath(new URL("./symbols-upload.mjs", import.meta.url));

const SECRETS = {
  SENTRY_AUTH_TOKEN: "sntrys_token",
  SENTRY_ORG: "purlis",
  SENTRY_PROJECT: "purlis-app",
};

/** A folder laid out as `download-artifact` leaves the build's symbols artifacts. */
function aDownload(files = []) {
  const root = mkdtempSync(join(tmpdir(), "symbols-"));
  const dir = join(root, "symbols");
  mkdirSync(dir);
  for (const file of files) {
    const path = join(dir, file);
    mkdirSync(join(path, ".."), { recursive: true });
    if (file.endsWith(".dSYM")) {
      mkdirSync(join(path, "Contents/Resources/DWARF"), { recursive: true });
      writeFileSync(join(path, "Contents/Resources/DWARF/x"), "dwarf");
    } else {
      writeFileSync(path, "debug");
    }
  }
  return { root, dir };
}

/** A `sentry-cli` that writes its arguments and environment to `calls.json` and exits `code`. */
function aSentryCli(root, code = 0) {
  const path = join(root, "sentry-cli");
  writeFileSync(
    path,
    `#!/usr/bin/env node
const { appendFileSync } = require("node:fs");
appendFileSync(${JSON.stringify(join(root, "calls.jsonl"))}, JSON.stringify({
  args: process.argv.slice(2),
  url: process.env.SENTRY_URL,
  token: process.env.SENTRY_AUTH_TOKEN,
}) + "\\n");
${code === 0 ? "" : `console.error("error: API request failed"); process.exit(${code});`}
`,
  );
  chmodSync(path, 0o755);
  return path;
}

function calls(root) {
  try {
    return readFileSync(join(root, "calls.jsonl"), "utf8")
      .trim()
      .split("\n")
      .filter(Boolean)
      .map((line) => JSON.parse(line));
  } catch {
    return [];
  }
}

/** Runs the tool as the job does: `node tools/symbols-upload.mjs <command> <dir>`. */
function run(command, { root, dir }, env = {}) {
  const summary = join(root, "summary.md");
  const output = join(root, "output.txt");
  writeFileSync(summary, "");
  writeFileSync(output, "");
  const result = spawnSync(process.execPath, [TOOL, command, dir], {
    encoding: "utf8",
    env: {
      PATH: process.env.PATH,
      GITHUB_STEP_SUMMARY: summary,
      GITHUB_OUTPUT: output,
      ...env,
    },
  });
  return {
    code: result.status,
    log: result.stdout + result.stderr,
    summary: readFileSync(summary, "utf8"),
    output: readFileSync(output, "utf8"),
  };
}

test("without the intake's secrets the job uploads nothing, stays green and says which are missing", () => {
  const download = aDownload(["purlis-macos-arm64-symbols-0.6.0/purlis-app.dSYM"]);
  const result = run("check", download, { SENTRY_ORG: "purlis" });
  assert.equal(result.code, 0, result.log);
  assert.match(result.output, /^upload=false$/m);
  assert.match(result.summary, /SENTRY_SYMBOLS_TOKEN/);
  assert.match(result.summary, /SENTRY_PROJECT/);
  assert.doesNotMatch(result.summary, /SENTRY_ORG\b/);
  assert.match(result.summary, /#5687/);
});

test("a build that saved no debug files uploads nothing, stays green and says the profile strips them", () => {
  // The release profile strips every binary (`strip = true`) until the native crash handler is
  // switched on (#5691, ruling of 2026-11-08), so today no build has a debug file to save.
  const download = aDownload();
  const result = run("check", download, SECRETS);
  assert.equal(result.code, 0, result.log);
  assert.match(result.output, /^upload=false$/m);
  assert.match(result.summary, /no debug files/i);
  assert.match(result.summary, /strip/);
});

test("a folder download-artifact never made counts as no debug files", () => {
  const { root } = aDownload();
  const result = run("check", { root, dir: join(root, "never-made") }, SECRETS);
  assert.equal(result.code, 0, result.log);
  assert.match(result.output, /^upload=false$/m);
  assert.match(result.summary, /no debug files/i);
});

test("with the secrets and a build's debug files, each one is found and the job goes on to upload", () => {
  const download = aDownload([
    "purlis-macos-arm64-symbols-0.6.0/purlis-app.dSYM",
    "purlis-macos-arm64-symbols-0.6.0/purlis.dSYM",
    "purlis-macos-arm64-symbols-0.6.0/notes.txt",
  ]);
  const result = run("check", download, SECRETS);
  assert.equal(result.code, 0, result.log);
  assert.match(result.output, /^upload=true$/m);
  assert.match(result.summary, /purlis-app\.dSYM/);
  assert.match(result.summary, /purlis\.dSYM/);
  assert.doesNotMatch(result.summary, /notes\.txt/);
});

test("every debug file goes to the project in Sentry's EU region, in one upload", () => {
  const download = aDownload([
    "purlis-macos-arm64-symbols-0.6.0/purlis-app.dSYM",
    "purlis-macos-arm64-symbols-0.6.0/purlis.dSYM",
  ]);
  const cli = aSentryCli(download.root);
  const result = run("upload", download, { ...SECRETS, SENTRY_CLI: cli });
  assert.equal(result.code, 0, result.log);
  const [call, ...more] = calls(download.root);
  assert.deepEqual(more, []);
  assert.equal(call.url, "https://de.sentry.io");
  assert.equal(call.token, "sntrys_token");
  assert.deepEqual(call.args, [
    "debug-files",
    "upload",
    "--org",
    "purlis",
    "--project",
    "purlis-app",
    join(download.dir, "purlis-macos-arm64-symbols-0.6.0/purlis-app.dSYM"),
    join(download.dir, "purlis-macos-arm64-symbols-0.6.0/purlis.dSYM"),
  ]);
  assert.match(result.summary, /Uploaded 2/);
});

test("a failed upload fails the job, says the release was not held back, and that a rerun tries again", () => {
  const download = aDownload(["purlis-macos-arm64-symbols-0.6.0/purlis-app.dSYM"]);
  const cli = aSentryCli(download.root, 1);
  const result = run("upload", download, { ...SECRETS, SENTRY_CLI: cli });
  assert.notEqual(result.code, 0);
  assert.match(result.log, /::warning::/);
  assert.match(result.summary, /not held back/);
  assert.match(result.summary, /[Rr]erun/);
});
