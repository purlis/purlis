// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **The image every story picture is drawn in** (#2901, #2942, `docs/component-stories.md`):
// Playwright's own Linux image for the Playwright `app/package-lock.json` locks, named by its tag
// and its digest. A tag can be pushed again; a digest names one image for good, so a baseline is
// compared in the very image it was drawn in.
//
//   node tools/story-image.mjs     every Playwright image `stories.yml` runs in is
//                                  `mcr.microsoft.com/playwright:v<locked>-noble@sha256:<digest>`,
//                                  with one digest for all of them
//
// `stories.yml` both compares the pictures and draws the baselines a pull request takes, in one
// job (`stories-baselines.yml` draws nothing). When the lockfile's Playwright moves, its image
// moves to the new version's tag and its digest, which
// `docker buildx imagetools inspect mcr.microsoft.com/playwright:v<version>-noble` prints.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

/** The workflows whose jobs draw and compare the pictures. */
export const WORKFLOWS = [".github/workflows/stories.yml"];

/** A job's `image:` that names Playwright's image, with what follows it. */
const USE =
  /^\s*image:\s*["']?(mcr\.microsoft\.com\/playwright\S*?)["']?\s*(?:#.*)?$/;

/** A use pinned as it must be: the version's tag and the digest. */
const PINNED =
  /^mcr\.microsoft\.com\/playwright:v(\d+\.\d+\.\d+)-noble@(sha256:[0-9a-f]{64})$/;

/** The Playwright version a parsed `package-lock.json` locks. */
export function lockedPlaywright(lock) {
  const version = lock?.packages?.["node_modules/playwright"]?.version;
  if (typeof version !== "string")
    throw new Error("app/package-lock.json locks no Playwright");
  return version;
}

/** What is wrong with the Playwright images `workflows` (each `{ name, text }`) run in, against
 *  the `locked` Playwright version, one line each. */
export function imageProblems(locked, workflows) {
  const wanted = `mcr.microsoft.com/playwright:v${locked}-noble@sha256:<digest>`;
  const found = [];
  let first;
  for (const { name, text } of workflows) {
    const uses = text
      .split("\n")
      .map((line, at) => ({ line: at + 1, match: USE.exec(line) }))
      .filter(({ match }) => match);
    if (uses.length === 0) {
      found.push(`${name} names no Playwright image; it must run in ${wanted}`);
      continue;
    }
    for (const { line, match } of uses) {
      const image = match[1];
      const where = `${name}:${line}`;
      const pinned = PINNED.exec(image);
      if (!pinned) {
        found.push(
          `${where}: ${image} is not named by its tag and its digest; it must be ${wanted}`,
        );
        continue;
      }
      const [, version, digest] = pinned;
      if (version !== locked) {
        found.push(
          `${where}: app/package-lock.json locks Playwright ${locked}, and the image is ${version}'s: ` +
            `move both workflows' image to ${wanted}`,
        );
        continue;
      }
      if (!first) first = { where, digest };
      else if (digest !== first.digest)
        found.push(
          `${where}: the image's digest is ${digest}, and ${first.where} names ${first.digest}: ` +
            "every story is drawn and compared in one image",
        );
    }
  }
  return found;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const root = join(dirname(fileURLToPath(import.meta.url)), "..");
  const read = (path) => readFileSync(join(root, path), "utf8");
  const locked = lockedPlaywright(JSON.parse(read("app/package-lock.json")));
  const found = imageProblems(
    locked,
    WORKFLOWS.map((path) => ({ name: path, text: read(path) })),
  );
  for (const line of found) console.error(`::error::${line}`);
  if (found.length > 0) process.exit(1);
  console.log(
    `the stories image is Playwright ${locked}'s, by its tag and one digest`,
  );
}
