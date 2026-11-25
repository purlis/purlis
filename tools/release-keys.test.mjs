// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// What `release.yml`'s `keys` job decides about macOS signing and notarization (ADR 0042, amended
// 2026-11-25), run as the shell it is, once per combination of which secrets are set. The job
// reads each secret as a boolean, so the cases are the booleans. Run with `node --test tools/`.
//
// A stable build is either ad-hoc with none of the six Apple secrets, or Developer ID signed and
// notarized with all six; every other combination stops it, loudly, before twenty minutes of
// building. A dev build is never notarized, whatever is set, but a half-set group is still refused.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const lines = readFileSync(new URL("../.github/workflows/release.yml", import.meta.url), "utf8").split("\n");

/** The `run:` script of the `keys` job's one step, dedented as the runner hands it to bash. */
function keysScript() {
  const start = lines.indexOf("  keys:");
  assert.notEqual(start, -1, "no keys job");
  const end = lines.findIndex((line, i) => i > start && /^ {2}\S/.test(line));
  const job = lines.slice(start, end === -1 ? undefined : end);
  const run = job.findIndex((line) => line === "        run: |");
  assert.notEqual(run, -1, "keys has no run script");
  const body = [];
  for (const line of job.slice(run + 1)) {
    if (line.trim() !== "" && !line.startsWith("          ")) break;
    body.push(line.slice(10));
  }
  return body.join("\n");
}

const DIR = mkdtempSync(join(tmpdir(), "release-keys-"));
// `gh release view dev` answers that `dev` is a prerelease: the one thing the dev branch asks.
writeFileSync(join(DIR, "gh"), "#!/bin/sh\necho true\n");
chmodSync(join(DIR, "gh"), 0o755);
writeFileSync(join(DIR, "keys.sh"), keysScript());

/** Runs the script for a channel with `devId` (0 to 3) and `api` (0 to 3) of each group set. */
function run(channel, devId, api) {
  const out = join(DIR, `out-${channel}-${devId}-${api}`);
  writeFileSync(out, "");
  const flag = (n, of) => String(n >= of);
  const result = spawnSync("bash", [join(DIR, "keys.sh")], {
    timeout: 20_000,
    encoding: "utf8",
    env: {
      PATH: `${DIR}:${process.env.PATH}`,
      GITHUB_OUTPUT: out,
      GH_TOKEN: "x",
      GITHUB_REPOSITORY: "purlis/purlis",
      DEV: String(channel === "dev"),
      PUBLISH: "true",
      CHANNEL: channel,
      HAS_UPDATER_KEY: "true",
      HAS_CERT: flag(devId, 1),
      HAS_CERT_PASSWORD: flag(devId, 2),
      HAS_IDENTITY: flag(devId, 3),
      HAS_API_ISSUER: flag(api, 1),
      HAS_API_KEY: flag(api, 2),
      HAS_API_KEY_P8: flag(api, 3),
    },
  });
  assert.equal(result.error, undefined, String(result.error));
  const outputs = Object.fromEntries(
    readFileSync(out, "utf8")
      .split("\n")
      .filter(Boolean)
      .map((line) => line.split(/=(.*)/s).slice(0, 2)),
  );
  return { status: result.status, outputs, log: `${result.stdout}${result.stderr}` };
}

test("a stable build with none of the Apple secrets is ad-hoc signed and not notarized", () => {
  const { status, outputs } = run("stable", 0, 0);
  assert.equal(status, 0);
  assert.equal(outputs.macos_signing, "ad-hoc");
  assert.equal(outputs.notarize, "false");
});

test("a stable build with the Developer ID and the whole notarization key is signed and notarized", () => {
  const { status, outputs, log } = run("stable", 3, 3);
  assert.equal(status, 0, log);
  assert.equal(outputs.macos_signing, "developer-id");
  assert.equal(outputs.notarize, "true");
});

test("a stable build with a Developer ID and no notarization key is refused, saying what to set", () => {
  const { status, log } = run("stable", 3, 0);
  assert.equal(status, 1);
  assert.match(log, /a stable build is notarized, and the Developer ID is set without APPLE_API_ISSUER/);
});

test("a stable build with the notarization key and no Developer ID is refused", () => {
  const { status, log } = run("stable", 0, 3);
  assert.equal(status, 1);
  assert.match(log, /set without a Developer ID, and nothing ad-hoc signed can be notarized/);
});

test("a stable build with half a Developer ID, or half a notarization key, is refused", () => {
  for (const [devId, api, said] of [
    [1, 0, /Developer ID secrets are half set/],
    [2, 3, /Developer ID secrets are half set/],
    [3, 1, /notarization secrets are half set/],
    [3, 2, /notarization secrets are half set/],
    [0, 2, /notarization secrets are half set/],
  ]) {
    const { status, log } = run("stable", devId, api);
    assert.equal(status, 1, `devId=${devId} api=${api}: ${log}`);
    assert.match(log, said, `devId=${devId} api=${api}`);
  }
});

test("a dev build is never notarized, whatever is set, and is Developer ID signed when it can be", () => {
  for (const [devId, api, signing] of [
    [0, 0, "ad-hoc"],
    [3, 0, "developer-id"],
    [3, 3, "developer-id"],
    [0, 3, "ad-hoc"],
  ]) {
    const { status, outputs, log } = run("dev", devId, api);
    assert.equal(status, 0, `devId=${devId} api=${api}: ${log}`);
    assert.equal(outputs.macos_signing, signing, `devId=${devId} api=${api}`);
    assert.equal(outputs.notarize, "false", `devId=${devId} api=${api}`);
  }
});

test("a dev build with half a Developer ID or half a notarization key is still refused", () => {
  assert.equal(run("dev", 2, 0).status, 1);
  assert.equal(run("dev", 3, 2).status, 1);
});
