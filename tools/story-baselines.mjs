// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **The story baselines** (#2901, `docs/component-stories.md`): the pictures each story is
// compared with, drawn only in the pinned Playwright image of `.github/workflows/stories.yml`.
//
//   node tools/story-baselines.mjs check            every baseline is one that image drew, of a
//                                                   story file that exists (the stories job)
//   node tools/story-baselines.mjs gather <out>     copy every baseline to <out>, under its path
//                                                   in `app/` (the labelled job, after drawing)
//   node tools/story-baselines.mjs take <drawn> <app>
//                                                   write what `gather` copied into <app> (the
//                                                   labelled job's commit, run from a trusted
//                                                   checkout against the pull request's)
//
// A baseline is `app/src/<dir>/__screenshots__/<name>.stories.tsx/<story id>-<browser>-linux.png`,
// where Vitest's `toMatchScreenshot` keeps it (`app/.storybook/screenshots.setup.ts`).
import {
  constants,
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { crc32 } from "node:zlib";

/** A baseline's path inside `app/`, with `/` between its parts. */
const BASELINE =
  /^src\/(?:[\w.-]+\/)*__screenshots__\/[\w.-]+\.stories\.tsx\/[\w.-]+-(?:chromium|webkit)-linux\.png$/;

/** How much `take` takes at most: bytes in one baseline, baselines in one run. A story's
 *  picture is tens of kilobytes; a dialog's on the page, a few hundred. */
export const LIMITS = { bytes: 2 * 1024 * 1024, files: 500 };

/** The widest and tallest picture a story is drawn at, in pixels. */
const SIDE = 4096;

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

/** Whether `bytes` are one whole PNG: the signature, an IHDR first with a size in bounds,
 *  every chunk inside the file with its CRC right, and IEND last, at the very end. */
export function isWholePng(bytes) {
  if (bytes.length < PNG.length || !bytes.subarray(0, PNG.length).equals(PNG))
    return false;
  let at = PNG.length;
  let first = true;
  while (at + 12 <= bytes.length) {
    const length = bytes.readUInt32BE(at);
    const end = at + 12 + length;
    if (end > bytes.length) return false;
    const type = bytes.toString("latin1", at + 4, at + 8);
    if (
      crc32(bytes.subarray(at + 4, at + 8 + length)) !==
      bytes.readUInt32BE(at + 8 + length)
    )
      return false;
    if (first) {
      if (type !== "IHDR" || length !== 13) return false;
      const width = bytes.readUInt32BE(at + 8);
      const height = bytes.readUInt32BE(at + 12);
      if (width < 1 || height < 1 || width > SIDE || height > SIDE)
        return false;
      first = false;
    }
    if (type === "IEND") return length === 0 && end === bytes.length;
    at = end;
  }
  return false;
}

/** Throws when any part of `path` under `root` that exists is a link, so nothing is read or
 *  written anywhere but inside `root`. */
function confined(root, path) {
  let at = root;
  for (const part of path.split("/")) {
    if (part === "" || part === "." || part === "..")
      throw new Error(
        `not a baseline, so nothing was taken: ${path} leaves its directory`,
      );
    at = join(at, part);
    let stat;
    try {
      stat = lstatSync(at);
    } catch {
      return;
    }
    if (stat.isSymbolicLink())
      throw new Error(
        `not a baseline, so nothing was taken: ${path} goes through a link`,
      );
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
 * `drawn` must be a baseline: a path a baseline has, a plain file reached through no link, a
 * whole PNG within {@link LIMITS}; and nothing in `app` it would be written to may be a link.
 * Unless all of it is, nothing is written.
 */
export function take(drawn, app, limits = LIMITS) {
  const all = files(drawn).sort();
  if (all.length > limits.files)
    throw new Error(
      `not a baseline, so nothing was taken: more than ${limits.files} files`,
    );
  const taken = all.map((path) => {
    if (!BASELINE.test(path))
      throw new Error(`not a baseline, so nothing was taken: ${path}`);
    confined(drawn, path);
    confined(app, path);
    const from = join(drawn, path);
    if (!lstatSync(from).isFile())
      throw new Error(`not a baseline, so nothing was taken: ${path}`);
    if (lstatSync(from).size > limits.bytes)
      throw new Error(
        `not a baseline, so nothing was taken: ${path} is over the cap of ${limits.bytes} bytes`,
      );
    const bytes = readFileSync(from);
    if (!isWholePng(bytes))
      throw new Error(
        `not a baseline, so nothing was taken: ${path} is not a whole PNG`,
      );
    return { path, bytes };
  });
  for (const { path, bytes } of taken) {
    const to = join(app, path);
    mkdirSync(dirname(to), { recursive: true });
    // Checked again after the directories are made, and written without following a link.
    confined(app, path);
    writeFileSync(to, bytes, {
      flag:
        constants.O_WRONLY |
        constants.O_CREAT |
        constants.O_TRUNC |
        constants.O_NOFOLLOW,
    });
  }
  return all;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const app = resolve(dirname(fileURLToPath(import.meta.url)), "..", "app");
  const [command, dir, into] = process.argv.slice(2);
  if (command === "check") {
    const found = problems(app);
    for (const line of found) console.error(`::error::${line}`);
    if (found.length > 0) process.exit(1);
    console.log(
      `${baselinesIn(app).length} baselines, each drawn by the pinned image`,
    );
  } else if (command === "gather" && dir) {
    for (const path of gather(app, resolve(dir))) console.log(path);
  } else if (command === "take" && dir && into) {
    for (const path of take(resolve(dir), resolve(into))) console.log(path);
  } else {
    console.error(
      "usage: story-baselines.mjs check | gather <out> | take <drawn> <app>",
    );
    process.exit(2);
  }
}
