// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The debug symbols a release build saved, sent to the Report intake (#5692, spec #5683): what
// `release.yml`'s `symbols` job runs, after the build and apart from it, so the build's time is
// untouched and a failed upload never holds the release back.
//
//   node tools/symbols-upload.mjs check  <dir>   # may this job upload, and is there anything to?
//   node tools/symbols-upload.mjs upload <dir>   # send every debug file under <dir>
//
// `<dir>` is where `download-artifact` left the build's `purlis-*-symbols-<version>` artifacts.
// `check` writes `upload=true|false` to GITHUB_OUTPUT; either way it exits 0 and says in the
// step summary why. `upload` runs `sentry-cli debug-files upload` (the binary named by
// SENTRY_CLI) against Sentry's EU region and exits non-zero when it fails, which the job's
// `continue-on-error` turns into a note on a green run: rerun the job to try again.
//
// The intake's names and token are the `release` environment's secrets, which the operator adds
// with the intake itself (#5687): SENTRY_SYMBOLS_TOKEN (an organisation token that may upload
// debug files), SENTRY_ORG and SENTRY_PROJECT. The job hands them over as SENTRY_AUTH_TOKEN,
// SENTRY_ORG and SENTRY_PROJECT, the names sentry-cli reads.
import { spawnSync } from "node:child_process";
import { appendFileSync, readdirSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

/** The intake's region: Sentry SaaS in the EU (ruling O9, ADR 0083 amended 2026-11-07). */
export const SENTRY_URL = "https://de.sentry.io";

/** Each secret the job needs: the variable this tool reads, and the secret's own name. */
const SECRETS = [
  ["SENTRY_AUTH_TOKEN", "SENTRY_SYMBOLS_TOKEN"],
  ["SENTRY_ORG", "SENTRY_ORG"],
  ["SENTRY_PROJECT", "SENTRY_PROJECT"],
];

/** The `release` environment's secrets missing from `env`, by the secret's own name. */
export function missingSecrets(env) {
  return SECRETS.filter(([variable]) => !env[variable]).map(([, secret]) => secret);
}

/** What a debug file is called: a macOS dSYM bundle, a Windows PDB, a Linux debug file. */
const DEBUG_FILE = /\.(dSYM|pdb|dwp|debug)$/;

/**
 * Every debug file under `dir`, relative to it and sorted: a `.dSYM` is a folder and counts as
 * one. A `dir` that does not exist has none: `download-artifact` makes no folder when no build
 * saved a symbols artifact.
 */
export function debugFiles(dir) {
  const found = [];
  const walk = (at) => {
    let entries;
    try {
      entries = readdirSync(at, { withFileTypes: true });
    } catch (error) {
      if (error.code === "ENOENT") return;
      throw error;
    }
    for (const entry of entries) {
      const path = join(at, entry.name);
      if (DEBUG_FILE.test(entry.name)) found.push(relative(dir, path));
      else if (entry.isDirectory()) walk(path);
    }
  };
  walk(dir);
  return found.sort();
}

function say(env, lines) {
  for (const line of lines) console.log(line);
  if (env.GITHUB_STEP_SUMMARY) appendFileSync(env.GITHUB_STEP_SUMMARY, `${lines.join("\n")}\n`);
}

function output(env, key, value) {
  if (env.GITHUB_OUTPUT) appendFileSync(env.GITHUB_OUTPUT, `${key}=${value}\n`);
}

function check(dir, env) {
  const missing = missingSecrets(env);
  if (missing.length > 0) {
    say(env, [
      "## Debug symbols",
      "",
      `Not uploaded: the \`release\` environment has no ${missing.map((s) => `\`${s}\``).join(", ")} ` +
        "yet. They come with the Report intake (#5687); until then this job skips, and the release is not held back.",
    ]);
    output(env, "upload", "false");
    return 0;
  }
  const files = debugFiles(dir);
  if (files.length === 0) {
    say(env, [
      "## Debug symbols",
      "",
      "Not uploaded: the build saved no debug files. The release profile strips every binary " +
        "(`strip = true`) and makes none, until the native crash handler is switched on (#5691).",
    ]);
    output(env, "upload", "false");
    return 0;
  }
  say(env, ["## Debug symbols", "", `Found ${files.length}:`, "", ...files.map((f) => `- \`${f}\``)]);
  output(env, "upload", "true");
  return 0;
}

function upload(dir, env) {
  const files = debugFiles(dir);
  const args = ["debug-files", "upload", "--org", env.SENTRY_ORG, "--project", env.SENTRY_PROJECT];
  const sent = spawnSync(env.SENTRY_CLI || "sentry-cli", [...args, ...files.map((f) => join(dir, f))], {
    stdio: "inherit",
    env: { ...env, SENTRY_URL },
  });
  if (sent.status === 0) {
    say(env, ["", `Uploaded ${files.length} to the Report intake.`]);
    return 0;
  }
  const why = sent.error ? sent.error.message : `sentry-cli exited ${sent.status ?? sent.signal}`;
  console.log(`::warning::Debug symbols were not uploaded (${why}); the release is not held back`);
  say(env, [
    "",
    `**Not uploaded** (${why}). The release is not held back by it. ` +
      "Rerun this job to try again: the build's symbols artifacts are kept 90 days.",
  ]);
  return 1;
}

export function main(argv = process.argv.slice(2), env = process.env) {
  const [command, dir] = argv;
  if (command === "check" && dir) return check(dir, env);
  if (command === "upload" && dir) return upload(dir, env);
  console.error("usage: symbols-upload.mjs check|upload <dir>");
  return 2;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exitCode = main();
