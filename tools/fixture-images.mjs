#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **The remote parity job's other machines, pulled by digest** (#7983). The images of the sshd,
// rootless Podman's sshd, the machine without Docker and the machine that boots systemd are
// built once, from main, by `.github/workflows/parity-fixture-images.yml`, pushed to the
// organisation's registry, and pinned by digest in `tests/parity/fixture-images.json` beside the
// source they were built from: their Dockerfile, the files it copies and its build arguments. So
// a run of `remote-parity` pulls them and never asks Alpine's CDN or Ubuntu's mirror for a
// package. A fixture whose files a change has moved past its pin, or one not pinned yet, is built
// here instead, as before, and says so.
//
//   node tools/fixture-images.mjs prepare-all
//       Each fixture's image, tagged with its name for its `up.sh`: pulled by digest when its pin
//       is current, built here otherwise. A fixture that cannot be had is one `::error::` line
//       naming it and its image, and the command exits 1 once every fixture was tried.
//   node tools/fixture-images.mjs stale
//       The fixtures whose pin is missing or behind their files, as a JSON list (the publish's
//       matrix).
//   node tools/fixture-images.mjs stage NAME DIR
//       NAME's Dockerfile and the files it copies, and nothing else, into DIR: the build context.
//   node tools/fixture-images.mjs build-args NAME
//       NAME's build arguments, one `KEY=VALUE` a line.
//   node tools/fixture-images.mjs pin NAME REF
//       Writes REF (`<image>@sha256:<digest>`) as NAME's pin, with the source it was built from.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { fileURLToPath } from "node:url";

export const MANIFEST = "tests/parity/fixture-images.json";
const ROOT = fileURLToPath(new URL("../", import.meta.url));
const repoRead = (path) => readFileSync(join(ROOT, path), "utf8");

/** The manifest, read through `readFile` (a repository path to its text). */
export function loadManifest(readFile = repoRead) {
  return JSON.parse(readFile(MANIFEST));
}

/** The sources a Dockerfile's `COPY` lines name (never their destination). */
export function copiedSources(dockerfile) {
  const out = [];
  for (const line of dockerfile.split("\n")) {
    const m = /^\s*COPY\s+(.*)$/i.exec(line);
    if (!m) continue;
    const words = m[1].trim().split(/\s+/).filter((w) => !w.startsWith("--"));
    out.push(...words.slice(0, -1));
  }
  return out;
}

/**
 * What a fixture's image is built from, as one digest: its Dockerfile, each file it copies (by
 * name and text) and its build arguments. The image's pin is current only while this is the
 * source the pin names.
 */
export function sourceOf(fixture, readFile = repoRead) {
  const hash = createHash("sha256");
  const part = (label, text) => hash.update(`${label}\0${text.length}\0${text}\0`);
  part("dockerfile", readFile(fixture.dockerfile));
  for (const file of [...fixture.files].sort()) part(`file:${file}`, readFile(`${fixture.context}/${file}`));
  const args = Object.entries(fixture.buildArgs ?? {}).sort(([a], [b]) => a.localeCompare(b));
  part("build-args", JSON.stringify(args));
  return `sha256:${hash.digest("hex")}`;
}

/** The manifest with `ref` as `name`'s pin. Throws for a ref that is not by digest. */
export function withPin(manifest, name, ref, readFile = repoRead) {
  const fixture = manifest.fixtures[name];
  if (!fixture) throw new Error(`no fixture ${name} in ${MANIFEST}`);
  if (!/^[a-z0-9./-]+@sha256:[0-9a-f]{64}$/.test(ref)) {
    throw new Error(`${ref} names no image by digest; a fixture is pinned by digest alone`);
  }
  const image = { ref, source: sourceOf(fixture, readFile) };
  return { ...manifest, fixtures: { ...manifest.fixtures, [name]: { ...fixture, image } } };
}

/** Whether a fixture's pin is there and current: `current`, `stale` or `unpinned`. */
export function pinState(fixture, readFile = repoRead) {
  if (!fixture.image) return "unpinned";
  return fixture.image.source === sourceOf(fixture, readFile) ? "current" : "stale";
}

/** Copies a fixture's Dockerfile and the files it copies into `dir`. */
function stageFiles(fixture, dir, readFile = repoRead) {
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, basename(fixture.dockerfile)), readFile(fixture.dockerfile));
  for (const file of fixture.files) {
    // Its mode as committed: an entrypoint stays executable.
    const from = join(ROOT, fixture.context, file);
    writeFileSync(join(dir, file), readFileSync(from), { mode: statSync(from).mode & 0o777 });
  }
}

const said = (error) => String(error?.stderr || error?.message || error).trim().split("\n").filter(Boolean).at(-1) ?? "no answer";
const sleep = (ms) => Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);

/**
 * Makes fixture `name`'s image here, tagged `tag`: `{ how, notice }`, where `how` is `pulled` or
 * `built`. `docker(args)` runs docker and returns its output or throws. Throws one message naming
 * the fixture when its image cannot be pulled or built.
 */
export function prepare({
  name,
  fixture,
  tag,
  readFile = repoRead,
  docker,
  tries = 3,
  wait = sleep,
  contextDir,
  stage = (f, dir) => stageFiles(f, dir, readFile),
}) {
  const state = pinState(fixture, readFile);
  if (state === "current") {
    const { ref } = fixture.image;
    let last;
    for (let attempt = 1; attempt <= tries; attempt += 1) {
      try {
        docker(["pull", ref]);
        docker(["tag", ref, tag]);
        return { how: "pulled", notice: `${name}: ${ref}, pulled by digest` };
      } catch (error) {
        last = error;
        if (attempt < tries) wait(attempt * 10_000);
      }
    }
    throw new Error(`fixture image ${name} (${ref}) unavailable after ${tries} tries: ${said(last)}`);
  }
  const why =
    state === "stale"
      ? `${fixture.dockerfile} or what it copies changed since it was pinned`
      : "it is not pinned yet";
  const dir = contextDir ?? mkdtempSync(join(tmpdir(), `fixture-${name}-`));
  stage(fixture, dir);
  const args = Object.entries(fixture.buildArgs ?? {}).flatMap(([k, v]) => ["--build-arg", `${k}=${v}`]);
  try {
    docker(["build", "--file", join(dir, basename(fixture.dockerfile)), "--tag", tag, ...args, dir]);
  } catch (error) {
    throw new Error(
      `fixture image ${name} could not be built from ${fixture.dockerfile} (built here because ${why}; a distro mirror may be down): ${said(error)}`,
    );
  }
  return { how: "built", notice: `${name}: built here from ${fixture.dockerfile}, because ${why}` };
}

function realDocker(args) {
  return execFileSync("docker", args, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], maxBuffer: 64 * 1024 * 1024 });
}

function main([command, ...rest]) {
  const manifest = loadManifest();
  const fixtureOf = (name) => {
    const fixture = manifest.fixtures[name];
    if (!fixture) throw new Error(`no fixture ${name} in ${MANIFEST}`);
    return fixture;
  };
  if (command === "prepare-all" && rest.length === 0) {
    let failed = 0;
    for (const [name, fixture] of Object.entries(manifest.fixtures)) {
      try {
        const { how, notice } = prepare({ name, fixture, tag: name, docker: realDocker });
        console.log(how === "pulled" ? notice : `::warning title=Fixture image built here::${notice}`);
      } catch (error) {
        failed += 1;
        console.log(`::error title=Fixture image unavailable::${error.message}`);
      }
    }
    if (failed > 0) process.exit(1);
  } else if (command === "stale" && rest.length === 0) {
    console.log(JSON.stringify(Object.keys(manifest.fixtures).filter((n) => pinState(manifest.fixtures[n]) !== "current")));
  } else if (command === "stage" && rest.length === 2) {
    stageFiles(fixtureOf(rest[0]), rest[1]);
  } else if (command === "build-args" && rest.length === 1) {
    for (const [k, v] of Object.entries(fixtureOf(rest[0]).buildArgs ?? {})) console.log(`${k}=${v}`);
  } else if (command === "pin" && rest.length === 2) {
    const next = withPin(manifest, rest[0], rest[1]);
    writeFileSync(join(ROOT, MANIFEST), `${JSON.stringify(next, null, 2)}\n`);
  } else {
    throw new Error("usage: fixture-images.mjs prepare-all | stale | stage NAME DIR | build-args NAME | pin NAME REF");
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    console.error(`::error::${error.message}`);
    process.exit(1);
  }
}
