// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **The story baselines** (#2901, `docs/component-stories.md`): the pictures each story is
// compared with, drawn only in the pinned Playwright image of `.github/workflows/stories.yml`.
//
//   node tools/story-baselines.mjs check            every baseline is one that image drew, of a
//                                                   story file that exists (the stories job)
//   node tools/story-baselines.mjs gather <out>     copy every baseline to <out>, under its path
//                                                   in `app/` (the labelled job, after drawing)
//   node tools/story-baselines.mjs take <drawn>     write what `gather` copied into `app/` (the
//                                                   labelled job's commit, which runs no app code)
//
// A baseline is `app/src/<dir>/__screenshots__/<name>.stories.tsx/<story id>-<browser>-linux.png`,
// where Vitest's `toMatchScreenshot` keeps it (`app/.storybook/screenshots.setup.ts`).
import {
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  openSync,
  readSync,
  closeSync,
  readdirSync,
} from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

/** A baseline's path inside `app/`, with `/` between its parts. */
const BASELINE =
  /^src\/(?:[\w.-]+\/)*__screenshots__\/[\w.-]+\.stories\.tsx\/[\w.-]+-(?:chromium|webkit)-linux\.png$/;

/** The eight bytes every PNG starts with. */
const PNG = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

/** Every file under `root`, as paths relative to it with `/` between their parts. */
function files(root, at = root) {
  if (!existsSync(at)) return [];
  return readdirSync(at, { withFileTypes: true }).flatMap((entry) => {
    const path = join(at, entry.name);
    if (entry.isDirectory()) return files(root, path);
    return [relative(root, path).split(sep).join("/")];
  });
}

/** Every file in a `__screenshots__` directory under `app/src`. */
function baselinesIn(app) {
  return files(join(app, "src"))
    .map((path) => `src/${path}`)
    .filter((path) => path.split("/").includes("__screenshots__"))
    .sort();
}

function startsLikeAPng(path) {
  const fd = openSync(path, "r");
  try {
    const head = Buffer.alloc(PNG.length);
    return (
      readSync(fd, head, 0, PNG.length, 0) === PNG.length && head.equals(PNG)
    );
  } finally {
    closeSync(fd);
  }
}

/** What is wrong with the baselines in `app`, one line each. */
export function problems(app) {
  return baselinesIn(app).flatMap((path) => {
    if (!BASELINE.test(path))
      return [
        `${path}: not a baseline the pinned image draws (<story id>-<chromium|webkit>-linux.png); ` +
          "baselines are drawn only by the update-baselines label (docs/component-stories.md)",
      ];
    const parts = path.split("/");
    const at = parts.indexOf("__screenshots__");
    const story = [...parts.slice(0, at), parts[at + 1]].join("/");
    if (!existsSync(join(app, story)))
      return [
        `${path}: its story file ${story} is gone; delete the baseline with it`,
      ];
    return [];
  });
}

/** Copies every baseline in `app` to `out`, under the same path, and says which. */
export function gather(app, out) {
  const all = baselinesIn(app).filter((path) => BASELINE.test(path));
  for (const path of all) {
    mkdirSync(dirname(join(out, path)), { recursive: true });
    copyFileSync(join(app, path), join(out, path));
  }
  return all;
}

/**
 * Writes the baselines `gather` copied to `drawn` into `app`, and says which. Everything in
 * `drawn` must be a baseline, a regular file that starts as a PNG does, or nothing is written.
 */
export function take(drawn, app) {
  const all = files(drawn);
  const refused = all.filter((path) => {
    if (!BASELINE.test(path)) return true;
    const stat = lstatSync(join(drawn, path));
    return !stat.isFile() || !startsLikeAPng(join(drawn, path));
  });
  if (refused.length > 0)
    throw new Error(
      `not a baseline, so nothing was taken: ${refused.join(", ")}`,
    );
  for (const path of all) {
    mkdirSync(dirname(join(app, path)), { recursive: true });
    copyFileSync(join(drawn, path), join(app, path));
  }
  return all.sort();
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const app = resolve(dirname(fileURLToPath(import.meta.url)), "..", "app");
  const [command, dir] = process.argv.slice(2);
  if (command === "check") {
    const found = problems(app);
    for (const line of found) console.error(`::error::${line}`);
    if (found.length > 0) process.exit(1);
    console.log(
      `${baselinesIn(app).length} baselines, each drawn by the pinned image`,
    );
  } else if (command === "gather" && dir) {
    for (const path of gather(app, resolve(dir))) console.log(path);
  } else if (command === "take" && dir) {
    for (const path of take(resolve(dir), app)) console.log(path);
  } else {
    console.error(
      "usage: story-baselines.mjs check | gather <out> | take <drawn>",
    );
    process.exit(2);
  }
}
