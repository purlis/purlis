// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **The image every story picture is drawn in** (#2901, #2942, #2956, `docs/component-stories.md`):
// Playwright's own Linux image for the Playwright `app/package-lock.json` locks, named by its tag
// and its digest. A tag can be pushed again; a digest names one image for good, so a baseline is
// compared in the very image it was drawn in.
//
// The pin lives in one file, `PIN`, which nothing builds: it is a Dockerfile so that Dependabot's
// `playwright` group (`.github/dependabot.yml`) moves its tag and digest in the one pull request
// that moves the npm packages. Dependabot does not read a workflow's `container:`, and the
// workflow's own token may not push a change to a workflow file. `stories.yml` reads the pin
// here, in a job of its own on the runner, as a job's container is pulled before its first step.
//
//   node tools/story-image.mjs       the pin is
//                                    `mcr.microsoft.com/playwright:v<locked>-noble@sha256:<digest>`,
//                                    `stories.yml` draws in it, and any workflow that names a
//                                    Playwright image names that one
//   node tools/story-image.mjs ref   prints the pinned image, for `stories.yml`'s job
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

/** The one place the image is named. */
export const PIN = "app/.storybook/playwright-image/Dockerfile";

/** The workflow that draws and compares the pictures. */
export const DRAWS = ".github/workflows/stories.yml";

/** How `DRAWS` names its image: what its job on the runner read from `PIN`. */
export const READS_THE_PIN = /^\s*image:\s*\$\{\{\s*needs\.changes\.outputs\.image\s*\}\}\s*(?:#.*)?$/m;

/** Every use of Playwright's image on a line, in any form: `image: x`, `container: x`,
 *  `{ image: x }`, `uses: docker://x`, or a command's argument. */
const USE = /mcr\.microsoft\.com\/playwright[^\s"'`,}\]]*/g;

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

/** The image `PIN`'s text names, whatever it is: its one `FROM`. */
export function pinnedImage(text) {
  const froms = text
    .split("\n")
    .map((line) => /^\s*FROM\s+(\S+)/i.exec(line))
    .filter(Boolean);
  if (froms.length !== 1)
    throw new Error(`${PIN} must have one FROM, and has ${froms.length}`);
  return froms[0][1];
}

/** A line without its YAML comment. */
const uncommented = (line) => line.replace(/(^|\s)#.*$/, "");

/** What is wrong with the pin's text `pin` and with the `workflows` (each `{ name, text }`),
 *  against the `locked` Playwright version, one line each. */
export function imageProblems(locked, pin, workflows) {
  const wanted = `mcr.microsoft.com/playwright:v${locked}-noble@sha256:<digest>`;
  const found = [];
  let image;
  try {
    image = pinnedImage(pin);
  } catch (error) {
    return [error.message];
  }
  const pinned = PINNED.exec(image);
  if (!pinned)
    found.push(`${PIN}: ${image} is not named by its tag and its digest; it must be ${wanted}`);
  else if (pinned[1] !== locked)
    found.push(
      `${PIN}: app/package-lock.json locks Playwright ${locked}, and the image is ${pinned[1]}'s: ` +
        `move it to ${wanted}`,
    );
  for (const { name, text } of workflows) {
    if (name === DRAWS && !READS_THE_PIN.test(text))
      found.push(
        `${name} does not draw in the pinned image: its job's image must be \${{ needs.changes.outputs.image }}`,
      );
    text.split("\n").forEach((line, at) => {
      for (const [use] of uncommented(line).matchAll(USE))
        if (use !== image)
          found.push(
            `${name}:${at + 1}: ${use} is not the image ${PIN} pins, ${image}: every story is drawn and compared in one image`,
          );
    });
  }
  return found;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const root = join(dirname(fileURLToPath(import.meta.url)), "..");
  const read = (path) => readFileSync(join(root, path), "utf8");
  if (process.argv[2] === "ref") {
    console.log(pinnedImage(read(PIN)));
    process.exit(0);
  }
  const locked = lockedPlaywright(JSON.parse(read("app/package-lock.json")));
  const found = imageProblems(
    locked,
    read(PIN),
    readdirSync(join(root, ".github/workflows"))
      .filter((file) => /\.ya?ml$/.test(file))
      .map((file) => `.github/workflows/${file}`)
      .map((path) => ({ name: path, text: read(path) })),
  );
  for (const line of found) console.error(`::error::${line}`);
  if (found.length > 0) process.exit(1);
  console.log(
    `the stories image is Playwright ${locked}'s, by its tag and its digest`,
  );
}
