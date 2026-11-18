#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **The Linux release-profile job's checks** (M74.1, T1.1-54, #6901). The scenario host is a test
// build; what a person runs is `release.yml`'s bundle. `app/src-tauri/src/scenarios/switches.json`
// lists each place the two differ (a switch or a substitution) with the cover for each OS, and
// every Linux entry that names `linux-release-profile` is checked here, once, against the bundle
// built as `release.yml` builds it (`.github/workflows/sandbox-scenarios.yml`, job
// `linux-release-profile`).
//
// An entry gets the check its kind of difference asks for, never a bundle-wide pass counted once:
//
// - `fence`: the entry is gated on the fence, the test seams or `cfg(test)`. The bundle's `purlis`
//   and `purlis-app` must be built with none of them: neither answers `_built-with`, which only a
//   debug build, or one with the fence or the test seams, does (`purlis_core::fence`).
// - `profile`: the entry is gated on `debug_assertions`. The bundle's `purlis` must say it was
//   built with the release profile (`purlis --version`), where debug assertions are off.
// - `bundle`: the entry is a substitution (what the scenario host stands in for). The bundle must
//   hold the real thing: the app, the `purlis` its hooks run, and the `.deb` that carries both.
//
// An entry this tool cannot give a check fails the run, so a new kind of switch is a red job and
// never a silent skip.
//
//     node tools/release-profile-checks.mjs plan                 # the entries and their checks
//     node tools/release-profile-checks.mjs run [<target dir>] [<profile>]

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
export const SWITCHES = join(root, "app/src-tauri/src/scenarios/switches.json");
export const COVER = "linux-release-profile";

/** The Linux entries of the list that name this job's cover, each with its check. */
export function entries(list = JSON.parse(readFileSync(SWITCHES, "utf8"))) {
  const out = [];
  for (const s of list.switches) {
    if (!(s.covers?.linux ?? []).includes(COVER)) continue;
    out.push({ id: `switch ${s.file}: ${s.at}`, check: checkOf(s) });
  }
  for (const s of list.substitutions) {
    if (!(s.covers?.linux ?? []).includes(COVER)) continue;
    out.push({ id: `substitution ${s.name}`, check: "bundle" });
  }
  return out;
}

/** The check a switch asks for, from what it is gated on. */
export function checkOf(s) {
  return /debug_assertions/.test(s.at) ? "profile" : "fence";
}

const binary = (target, profile, name) => join(target, profile, name);
const run = (program, args) => spawnSync(program, args, { encoding: "utf8", timeout: 60_000 });

/** The checks themselves: each returns `null` when it holds, else what is wrong. */
export const CHECKS = {
  fence({ target, profile }) {
    for (const name of ["purlis", "purlis-app"]) {
      const path = binary(target, profile, name);
      if (!existsSync(path)) return `${path} is not in the bundle's build`;
      const said = run(path, ["_built-with"]);
      if (said.error) return `${path}: ${said.error.message}`;
      if (/fenced=|test-support=/.test(`${said.stdout}${said.stderr}`)) {
        return `${name} answers _built-with, so it was built with the fence, the test seams or debug assertions: ${said.stdout.trim()}`;
      }
    }
    return null;
  },
  profile({ target, profile }) {
    const path = binary(target, profile, "purlis");
    const said = run(path, ["--version"]);
    if (said.error) return `${path}: ${said.error.message}`;
    const line = said.stdout.trim();
    return line.endsWith(`(${profile} profile)`) ? null : `purlis says "${line}", not a ${profile} profile build`;
  },
  bundle({ target, profile }) {
    for (const name of ["purlis", "purlis-app"]) {
      if (!existsSync(binary(target, profile, name))) return `${name} is not built`;
    }
    const folder = join(target, profile, "bundle", "deb");
    const debs = existsSync(folder) ? readdirSync(folder).filter((n) => n.endsWith(".deb")) : [];
    if (debs.length === 0) return `no .deb in ${folder}`;
    const listing = run("dpkg-deb", ["-c", join(folder, debs[0])]);
    if (listing.error || listing.status !== 0) return `dpkg-deb -c ${debs[0]}: ${listing.error?.message ?? listing.stderr}`;
    for (const name of ["purlis-app", "purlis"]) {
      if (!new RegExp(`/usr/bin/${name}\\s*$`, "m").test(listing.stdout)) {
        const held = listing.stdout.split("\n").filter((l) => /\/usr\/bin\//.test(l)).map((l) => l.trim().split(/\s+/).pop());
        return `${debs[0]} holds no /usr/bin/${name} (it holds ${held.join(", ") || "nothing in /usr/bin"})`;
      }
    }
    return null;
  },
};

/** Runs every entry's check against `target`/`profile`. `{ results: [{ id, check, problem }] }`. */
export function runAll({ target = "target", profile = "release", list } = {}) {
  const results = [];
  for (const entry of entries(list)) {
    const check = CHECKS[entry.check];
    const problem = check ? check({ target, profile }) : `no check named ${entry.check}`;
    results.push({ ...entry, problem });
  }
  return { results };
}

function main([command, target = "target", profile = "release"]) {
  if (command === "plan") {
    const all = entries();
    for (const e of all) console.log(`${e.check}\t${e.id}`);
    console.log(`${all.length} entries`);
    return 0;
  }
  if (command === "run") {
    const { results } = runAll({ target, profile });
    const red = results.filter((r) => r.problem);
    // Each check runs once per entry on purpose: the report says which entries a red check reaches.
    const out = join(target, "sandbox-scenarios", "linux-release-profile.json");
    mkdirSync(dirname(out), { recursive: true });
    writeFileSync(out, JSON.stringify(results, null, 2));
    console.log(`${results.length} entries checked against the ${profile} bundle: ${red.length} red`);
    for (const r of red) console.log(`::error title=release profile::${r.id}: ${r.problem}`);
    if (results.length === 0) {
      console.log("::error title=release profile::no entry names linux-release-profile: the list is not read");
      return 1;
    }
    return red.length === 0 ? 0 : 1;
  }
  console.error("usage: release-profile-checks.mjs plan|run [<target dir>] [<profile>]");
  return 2;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exitCode = main(process.argv.slice(2));
