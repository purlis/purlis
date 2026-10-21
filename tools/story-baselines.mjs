// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **The story baselines** (#2901, `docs/component-stories.md`): the pictures each story is
// compared with, drawn only in the pinned Playwright image of `.github/workflows/stories.yml`.
//
//   node tools/story-baselines.mjs check <index>    every baseline is one that image drew, of a
//                                                   story the index has (the stories job)
//   node tools/story-baselines.mjs gather <index> <out>
//                                                   copy the baseline of every story the index
//                                                   has to <out>, under its path in `app/` (the
//                                                   labelled job, after drawing)
//   node tools/story-baselines.mjs take <drawn> <app>
//                                                   write what `gather` copied into <app> and
//                                                   delete every other baseline there (the
//                                                   labelled job's commit, run from a trusted
//                                                   checkout against the pull request's)
//
// <index> is Storybook's own index of the stories, which `storybook index` writes. A story is
// live when the index has it with the `test` tag, the stories the Vitest addon runs; every other
// baseline is obsolete, and `check` fails on it, naming its story.
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
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, join, relative, resolve, sep } from "node:path";
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

/** The tag of the stories the Vitest addon runs, so the only ones a baseline is drawn of. */
const TESTED = "test";

/** The name Vitest gives a story's picture: its id with each run of dashes made one. */
export function pictureName(id) {
  return id
    .replace(/\s+/g, "-")
    .replace(/[^\w-]+/g, "")
    .replace(/-{2,}/g, "-");
}

/** The pictures each story file's live stories are drawn as: story file path in `app/` (with
 *  `/`), to the picture names of its stories that run as tests. */
export function liveStories(index) {
  if (
    typeof index !== "object" ||
    index === null ||
    typeof index.entries !== "object" ||
    index.entries === null
  )
    throw new Error(
      "not a story index: expected the JSON `storybook index` writes",
    );
  const live = new Map();
  for (const entry of Object.values(index.entries)) {
    if (entry?.type !== "story" || !entry.tags?.includes(TESTED)) continue;
    const file = String(entry.importPath).replace(/^\.\//, "");
    if (!live.has(file)) live.set(file, new Set());
    live.get(file).add(pictureName(String(entry.id)));
  }
  return live;
}

/** A baseline's story file, and the picture name of its story. */
function storyOf(path) {
  const parts = path.split("/");
  const at = parts.indexOf("__screenshots__");
  return {
    file: [...parts.slice(0, at), parts[at + 1]].join("/"),
    story: basename(path).replace(/-(?:chromium|webkit)-linux\.png$/, ""),
  };
}

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

/** What is wrong with the baselines in `app`, one line each, against Storybook's `index`. */
export function problems(app, index) {
  const live = liveStories(index);
  return baselinesIn(app).flatMap((path) => {
    if (!BASELINE.test(path))
      return [
        `${path}: not a baseline the pinned image draws (<story id>-<chromium|webkit>-linux.png); ` +
          "baselines are drawn only by the update-baselines label (docs/component-stories.md)",
      ];
    const { file, story } = storyOf(path);
    if (!existsSync(join(app, file)))
      return [
        `${path}: its story file ${file} is gone; delete the baseline with it`,
      ];
    if (!live.get(file)?.has(story))
      return [
        `${path}: no story ${story} in ${file} runs as a test, so the baseline is obsolete; ` +
          "the update-baselines label deletes it, or delete it with the story (docs/component-stories.md)",
      ];
    return [];
  });
}

/** Copies the baseline of every live story in `app` to `out`, under the same path, and says
 *  which: an obsolete one is left out, so `take` deletes it. */
export function gather(app, index, out) {
  const live = liveStories(index);
  const all = baselinesIn(app).filter((path) => {
    if (!BASELINE.test(path)) return false;
    const { file, story } = storyOf(path);
    return live.get(file)?.has(story) ?? false;
  });
  for (const path of all) {
    mkdirSync(dirname(join(out, path)), { recursive: true });
    copyFileSync(join(app, path), join(out, path));
  }
  return all;
}

/**
 * Writes the baselines `gather` copied to `drawn` into `app`, deletes every other file in a
 * `__screenshots__` directory of `app/src`, and says which it wrote and which it deleted.
 * Everything in `drawn` must be a baseline: a path a baseline has, a plain file reached through
 * no link, a whole PNG within {@link LIMITS}. Nothing in `app` written or deleted may be a link
 * or be reached through one, `app` included. Unless all of it holds, nothing is written or
 * deleted.
 */
export function take(drawn, app, limits = LIMITS) {
  if (lstatSync(app).isSymbolicLink())
    throw new Error(`not a baseline, so nothing was taken: ${app} is a link`);
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
  const drew = new Set(all);
  const removed = baselinesIn(app).filter((path) => !drew.has(path));
  for (const path of removed) {
    confined(app, path);
    if (!lstatSync(join(app, path)).isFile())
      throw new Error(
        `not a baseline, so nothing was taken: ${path} is not a plain file`,
      );
  }
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
  // `unlink` takes away the name, never what a link would point to; none is on the way here.
  for (const path of removed) unlinkSync(join(app, path));
  return { taken: all, removed };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const app = resolve(dirname(fileURLToPath(import.meta.url)), "..", "app");
  const [command, dir, into] = process.argv.slice(2);
  const indexAt = (path) => JSON.parse(readFileSync(resolve(path), "utf8"));
  if (command === "check" && dir) {
    const found = problems(app, indexAt(dir));
    for (const line of found) console.error(`::error::${line}`);
    if (found.length > 0) process.exit(1);
    console.log(
      `${baselinesIn(app).length} baselines, each drawn by the pinned image of a live story`,
    );
  } else if (command === "gather" && dir && into) {
    for (const path of gather(app, indexAt(dir), resolve(into)))
      console.log(path);
  } else if (command === "take" && dir && into) {
    const { taken, removed } = take(resolve(dir), resolve(into));
    for (const path of taken) console.log(`drawn: ${path}`);
    for (const path of removed) console.log(`deleted: ${path}`);
  } else {
    console.error(
      "usage: story-baselines.mjs check <index> | gather <index> <out> | take <drawn> <app>",
    );
    process.exit(2);
  }
}
