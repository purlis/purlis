// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The story baselines' own checks (#2901). Run with `node --test tools/`.
import assert from "node:assert/strict";
import {
  existsSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { crc32 } from "node:zlib";
import { join } from "node:path";
import { test } from "node:test";

import {
  BASELINE,
  gather,
  pictureName,
  picturedCells,
  problems,
  take,
  unpicturedIn,
} from "./story-baselines.mjs";
import { cellsOf, pictureOf } from "../app/.storybook/matrix.ts";

/** A chunk of a PNG: its length, type, data and CRC. */
function chunk(type, data) {
  const head = Buffer.alloc(4);
  head.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, "latin1"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([head, body, crc]);
}

const SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

/** A whole PNG, `width` by `height`: signature, IHDR, an IDAT, IEND. */
function png(width = 2, height = 2, extra = Buffer.alloc(0)) {
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header.set([8, 6, 0, 0, 0], 8);
  return Buffer.concat([
    SIGNATURE,
    chunk("IHDR", header),
    chunk(
      "IDAT",
      Buffer.from([0x78, 0x9c, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01]),
    ),
    chunk("IEND", Buffer.alloc(0)),
    extra,
  ]);
}

const PNG = png();
// Vitest names a picture by the story id with each run of dashes made one, then its cell.
const SHOT =
  "src/settings/__screenshots__/Choice.stories.tsx/settings-choice-radios_purlis_dark";

/** The cells pictures are taken in (`app/.storybook/pictured.json`). */
const CELLS = ["purlis_dark", "purlis-classic_light"];

/** Storybook's index (`storybook index`) of the stories with these ids in these files. */
function indexOf(stories) {
  const entries = {};
  for (const [id, file, tags = ["dev", "test"]] of stories)
    entries[id] = { type: "story", id, importPath: `./${file}`, tags };
  return { v: 5, entries };
}

const INDEX = indexOf([
  ["settings-choice--radios", "src/settings/Choice.stories.tsx"],
]);

function tree(files) {
  const root = mkdtempSync(join(tmpdir(), "story-baselines-"));
  for (const [path, body] of Object.entries(files)) {
    mkdirSync(join(root, path, ".."), { recursive: true });
    writeFileSync(join(root, path), body);
  }
  return root;
}

test("baselines drawn in the Linux image, beside their stories, pass", () => {
  const app = tree({
    "src/settings/Choice.stories.tsx": "",
    [`${SHOT}-chromium-linux.png`]: PNG,
    [`${SHOT}-webkit-linux.png`]: PNG,
  });
  assert.deepEqual(problems(app, INDEX, CELLS), []);
});

test("a story has a baseline in each pictured cell, named by its theme and appearance", () => {
  const app = tree({
    "src/settings/Choice.stories.tsx": "",
    [`${SHOT}-chromium-linux.png`]: PNG,
    "src/settings/__screenshots__/Choice.stories.tsx/settings-choice-radios_purlis-classic_light-webkit-linux.png":
      PNG,
  });
  assert.deepEqual(problems(app, INDEX, CELLS), []);
});

test("a baseline of a cell no picture is taken in is obsolete, naming its cell", () => {
  const app = tree({
    "src/settings/Choice.stories.tsx": "",
    "src/settings/__screenshots__/Choice.stories.tsx/settings-choice-radios_purlis_dark-high-contrast-webkit-linux.png":
      PNG,
  });
  const found = problems(app, INDEX, CELLS);
  assert.equal(found.length, 1);
  assert.match(
    found[0],
    /settings-choice-radios_purlis_dark-high-contrast .*pictured cell/,
  );
});

test("a baseline named for no cell is refused", () => {
  const app = tree({
    "src/settings/Choice.stories.tsx": "",
    "src/settings/__screenshots__/Choice.stories.tsx/settings-choice-radios-webkit-linux.png":
      PNG,
  });
  const found = problems(app, INDEX, CELLS);
  assert.equal(found.length, 1);
  assert.match(found[0], /<story id>_<theme>_<appearance>/);
});

test("the pictured cells are read from the app's matrix", () => {
  const app = tree({
    ".storybook/pictured.json": JSON.stringify(CELLS),
    "src/settings/Choice.stories.tsx": "",
    [`${SHOT}-webkit-linux.png`]: PNG,
  });
  assert.deepEqual(picturedCells(app), CELLS);
  assert.deepEqual(problems(app, INDEX), []);
});

test("a baseline drawn on another platform is refused, naming it", () => {
  const app = tree({
    "src/settings/Choice.stories.tsx": "",
    [`${SHOT}-chromium-darwin.png`]: PNG,
  });
  const found = problems(app, INDEX, CELLS);
  assert.equal(found.length, 1);
  assert.match(found[0], /chromium-darwin\.png/);
  assert.match(found[0], /pinned image/);
});

test("a baseline whose story file is gone is refused", () => {
  const app = tree({ [`${SHOT}-webkit-linux.png`]: PNG });
  const found = problems(app, INDEX, CELLS);
  assert.equal(found.length, 1);
  assert.match(found[0], /Choice\.stories\.tsx/);
});

test("gather copies every baseline under its path, and nothing else", () => {
  const app = tree({
    "src/settings/Choice.stories.tsx": "story",
    [`${SHOT}-webkit-linux.png`]: PNG,
  });
  const out = mkdtempSync(join(tmpdir(), "story-baselines-out-"));
  assert.deepEqual(gather(app, INDEX, out, CELLS), [
    `${SHOT}-webkit-linux.png`,
  ]);
  assert.deepEqual(readFileSync(join(out, `${SHOT}-webkit-linux.png`)), PNG);
  assert.equal(existsSync(join(out, "src/settings/Choice.stories.tsx")), false);
});

test("take writes drawn baselines into the app", () => {
  const drawn = tree({ [`${SHOT}-chromium-linux.png`]: PNG });
  const app = tree({ "src/settings/Choice.stories.tsx": "" });
  assert.deepEqual(take(drawn, app), {
    taken: [`${SHOT}-chromium-linux.png`],
    removed: [],
  });
  assert.deepEqual(readFileSync(join(app, `${SHOT}-chromium-linux.png`)), PNG);
});

test("take refuses anything that is not a baseline, and writes nothing", () => {
  for (const [path, body] of [
    ["src/settings/Choice.stories.tsx", "export {}"],
    [`${SHOT}-chromium-linux.png`, "not a picture"],
    [
      "src/settings/__screenshots__/Choice.stories.tsx/a-firefox-linux.png",
      PNG,
    ],
    [".github/workflows/x.yml", "on: push"],
  ]) {
    const drawn = mkdtempSync(join(tmpdir(), "story-baselines-drawn-"));
    const nested = join(drawn, "in");
    mkdirSync(join(nested, path, ".."), { recursive: true });
    writeFileSync(join(nested, path), body);
    const app = tree({ "src/settings/Choice.stories.tsx": "" });
    assert.throws(() => take(nested, app), /not a baseline/, path);
    assert.equal(
      existsSync(join(app, `${SHOT}-chromium-linux.png`)),
      false,
      path,
    );
  }
});

test("take refuses a link, even to a picture", () => {
  const elsewhere = tree({ "picture.png": PNG });
  const drawn = mkdtempSync(join(tmpdir(), "story-baselines-drawn-"));
  mkdirSync(join(drawn, SHOT, ".."), { recursive: true });
  symlinkSync(
    join(elsewhere, "picture.png"),
    join(drawn, `${SHOT}-chromium-linux.png`),
  );
  const app = tree({ "src/settings/Choice.stories.tsx": "" });
  assert.throws(() => take(drawn, app), /not a baseline/);
});

test("take refuses a file that starts as a PNG and is not a whole one", () => {
  const whole = png();
  for (const [why, body] of [
    ["the signature alone", SIGNATURE],
    ["no IEND", whole.subarray(0, whole.length - 12)],
    ["bytes after IEND", png(2, 2, Buffer.from("tail"))],
    [
      "a chunk with a wrong CRC",
      Buffer.concat([
        whole.subarray(0, 30),
        Buffer.from([0, 0, 0, 0]),
        whole.subarray(34),
      ]),
    ],
    ["no width", png(0, 2)],
    ["a width past the cap", png(100_000, 2)],
  ]) {
    const drawn = tree({ [`${SHOT}-chromium-linux.png`]: body });
    const app = tree({ "src/settings/Choice.stories.tsx": "" });
    assert.throws(() => take(drawn, app), /not a baseline/, why);
  }
});

test("take refuses a file over the size cap, and more files than the count cap", () => {
  const drawn = tree({
    [`${SHOT}-chromium-linux.png`]: PNG,
    [`${SHOT}-webkit-linux.png`]: PNG,
  });
  const app = tree({ "src/settings/Choice.stories.tsx": "" });
  assert.throws(
    () => take(drawn, app, { bytes: PNG.length - 1 }),
    /over the cap/,
  );
  assert.throws(() => take(drawn, app, { files: 1 }), /more than 1/);
  assert.equal(
    take(drawn, app, { bytes: PNG.length, files: 2 }).taken.length,
    2,
  );
});

test("take refuses to write through a link in the app, a file's or a directory's", () => {
  const elsewhere = tree({ "target.png": "kept" });
  const asFile = tree({ "src/settings/Choice.stories.tsx": "" });
  mkdirSync(join(asFile, SHOT, ".."), { recursive: true });
  symlinkSync(
    join(elsewhere, "target.png"),
    join(asFile, `${SHOT}-chromium-linux.png`),
  );
  const asDir = tree({ "src/settings/Choice.stories.tsx": "" });
  symlinkSync(elsewhere, join(asDir, "src/settings/__screenshots__"));
  for (const app of [asFile, asDir]) {
    const drawn = tree({ [`${SHOT}-chromium-linux.png`]: PNG });
    assert.throws(() => take(drawn, app), /a link/);
  }
  assert.equal(readFileSync(join(elsewhere, "target.png"), "utf8"), "kept");
});

test("a baseline whose story is gone from its file is refused, naming the story", () => {
  const app = tree({
    "src/settings/Choice.stories.tsx": "",
    [`${SHOT}-chromium-linux.png`]: PNG,
    "src/settings/__screenshots__/Choice.stories.tsx/settings-choice-checkboxes_purlis_dark-webkit-linux.png":
      PNG,
  });
  const found = problems(app, INDEX, CELLS);
  assert.equal(found.length, 1);
  assert.match(found[0], /story settings-choice-checkboxes_purlis_dark /);
  assert.match(found[0], /Choice\.stories\.tsx/);
});

test("a baseline is its own story's: one named for a story of another file is refused", () => {
  const app = tree({
    "src/settings/Choice.stories.tsx": "",
    "src/settings/Other.stories.tsx": "",
    "src/settings/__screenshots__/Other.stories.tsx/settings-choice-radios_purlis_dark-webkit-linux.png":
      PNG,
  });
  const found = problems(app, INDEX, CELLS);
  assert.equal(found.length, 1);
  assert.match(found[0], /story settings-choice-radios_purlis_dark /);
});

test("a baseline of a story that is not run as a test is refused", () => {
  const app = tree({
    "src/settings/Choice.stories.tsx": "",
    [`${SHOT}-webkit-linux.png`]: PNG,
  });
  const index = indexOf([
    ["settings-choice--radios", "src/settings/Choice.stories.tsx", ["dev"]],
  ]);
  const found = problems(app, index, CELLS);
  assert.equal(found.length, 1);
  assert.match(found[0], /story settings-choice-radios_purlis_dark /);
});

test("an index that is not Storybook's is refused", () => {
  const app = tree({ "src/settings/Choice.stories.tsx": "" });
  assert.throws(() => problems(app, {}, CELLS), /not a story index/);
  assert.throws(() => gather(app, null, app, CELLS), /not a story index/);
});

test("gather leaves out the baselines of stories that are gone", () => {
  const gone =
    "src/settings/__screenshots__/Choice.stories.tsx/settings-choice-gone_purlis_dark-webkit-linux.png";
  const app = tree({
    "src/settings/Choice.stories.tsx": "",
    [`${SHOT}-webkit-linux.png`]: PNG,
    [gone]: PNG,
  });
  const out = mkdtempSync(join(tmpdir(), "story-baselines-out-"));
  assert.deepEqual(gather(app, INDEX, out, CELLS), [
    `${SHOT}-webkit-linux.png`,
  ]);
  assert.equal(existsSync(join(out, gone)), false);
});

test("take deletes every baseline that was not drawn, and nothing else", () => {
  const gone =
    "src/settings/__screenshots__/Choice.stories.tsx/settings-choice-gone_purlis_dark-webkit-linux.png";
  const otherPlatform = `${SHOT}-webkit-darwin.png`;
  const drawn = tree({ [`${SHOT}-chromium-linux.png`]: PNG });
  const app = tree({
    "src/settings/Choice.stories.tsx": "story",
    "src/settings/picture.png": PNG,
    [`${SHOT}-chromium-linux.png`]: PNG,
    [gone]: PNG,
    [otherPlatform]: PNG,
  });
  assert.deepEqual(take(drawn, app), {
    taken: [`${SHOT}-chromium-linux.png`],
    removed: [gone, otherPlatform].sort(),
  });
  assert.equal(existsSync(join(app, gone)), false);
  assert.equal(existsSync(join(app, otherPlatform)), false);
  assert.equal(existsSync(join(app, `${SHOT}-chromium-linux.png`)), true);
  assert.equal(existsSync(join(app, "src/settings/picture.png")), true);
  assert.equal(
    readFileSync(join(app, "src/settings/Choice.stories.tsx"), "utf8"),
    "story",
  );
});

test("take deletes nothing through a link, and writes nothing when it would", () => {
  const elsewhere = tree({
    "__screenshots__/A.stories.tsx/a-webkit-linux.png": PNG,
  });
  const drawn = tree({ [`${SHOT}-chromium-linux.png`]: PNG });
  const app = tree({ "src/settings/Choice.stories.tsx": "" });
  mkdirSync(join(app, "src/linked"), { recursive: true });
  symlinkSync(
    join(elsewhere, "__screenshots__"),
    join(app, "src/linked/__screenshots__"),
  );
  assert.throws(() => take(drawn, app), /a link/);
  assert.equal(
    existsSync(
      join(elsewhere, "__screenshots__/A.stories.tsx/a-webkit-linux.png"),
    ),
    true,
  );
  assert.equal(existsSync(join(app, `${SHOT}-chromium-linux.png`)), false);
});

test("take refuses an app that is itself a link", () => {
  const real = tree({
    "src/settings/Choice.stories.tsx": "",
    [`${SHOT}-webkit-linux.png`]: PNG,
  });
  const holder = mkdtempSync(join(tmpdir(), "story-baselines-holder-"));
  symlinkSync(real, join(holder, "app"));
  const drawn = tree({ [`${SHOT}-chromium-linux.png`]: PNG });
  assert.throws(() => take(drawn, join(holder, "app")), /a link/);
  assert.equal(existsSync(join(real, `${SHOT}-webkit-linux.png`)), true);
});

test("every cell of the matrix can be pictured: its picture's name is a baseline's", () => {
  const themes = JSON.parse(
    readFileSync(
      new URL("../app/.storybook/built-in-themes.json", import.meta.url),
      "utf8",
    ),
  );
  const cells = cellsOf(themes, []);
  assert.ok(cells.some((cell) => cell.density !== "comfortable"));
  for (const cell of cells) {
    const id = "ui-button--primary";
    // As Vitest names the picture, and as `liveStories` expects it.
    const picture = pictureName(pictureOf(id, cell));
    assert.equal(picture, `${pictureName(id)}_${cell.name}`, cell.name);
    for (const browser of ["chromium", "webkit"])
      assert.match(
        `src/ui/button/__screenshots__/Button.stories.tsx/${picture}-${browser}-linux.png`,
        BASELINE,
        cell.name,
      );
  }
});

test("a compact cell's baseline is live where that cell is pictured", () => {
  const compact = `${SHOT}_compact-webkit-linux.png`;
  const app = tree({ "src/settings/Choice.stories.tsx": "", [compact]: PNG });
  assert.deepEqual(problems(app, INDEX, ["purlis_dark_compact"]), []);
  assert.equal(problems(app, INDEX, CELLS).length, 1);
});

test("take refuses baselines over the total cap, and writes nothing", () => {
  const drawn = tree({
    [`${SHOT}-chromium-linux.png`]: PNG,
    [`${SHOT}-webkit-linux.png`]: PNG,
  });
  const app = tree({ "src/settings/Choice.stories.tsx": "" });
  assert.throws(
    () => take(drawn, app, { total: 2 * PNG.length - 1 }),
    /in all/,
  );
  assert.equal(existsSync(join(app, `${SHOT}-chromium-linux.png`)), false);
  assert.equal(take(drawn, app, { total: 2 * PNG.length }).taken.length, 2);
});

test("a baseline of a story that takes no picture is obsolete, naming its story", () => {
  const app = tree({
    "src/settings/Choice.stories.tsx": "",
    [`${SHOT}-chromium-linux.png`]: PNG,
    [`${SHOT}-webkit-linux.png`]: PNG,
  });
  const off = unpicturedIn([
    {
      unpictured: [
        {
          file: "src/settings/Choice.stories.tsx",
          id: "settings-choice--radios",
        },
      ],
    },
  ]);
  const found = problems(app, INDEX, CELLS, off);
  assert.equal(found.length, 2);
  for (const line of found)
    assert.match(
      line,
      /settings-choice-radios_purlis_dark .*screenshot: false.*obsolete/,
    );
  assert.deepEqual(problems(app, INDEX, CELLS, []), []);
});

test("a story that takes no picture in one file leaves a story of that id in another alone", () => {
  const app = tree({
    "src/settings/Choice.stories.tsx": "",
    [`${SHOT}-webkit-linux.png`]: PNG,
  });
  const off = [
    { file: "src/other/Choice.stories.tsx", id: "settings-choice--radios" },
  ];
  assert.deepEqual(problems(app, INDEX, CELLS, off), []);
});

test("gather leaves out the baselines of a story that takes no picture", () => {
  const kept =
    "src/settings/__screenshots__/Choice.stories.tsx/settings-choice-checks_purlis_dark-webkit-linux.png";
  const index = indexOf([
    ["settings-choice--radios", "src/settings/Choice.stories.tsx"],
    ["settings-choice--checks", "src/settings/Choice.stories.tsx"],
  ]);
  const app = tree({
    "src/settings/Choice.stories.tsx": "",
    [`${SHOT}-webkit-linux.png`]: PNG,
    [kept]: PNG,
  });
  const out = mkdtempSync(join(tmpdir(), "story-baselines-out-"));
  const off = [
    {
      file: "./src/settings/Choice.stories.tsx",
      id: "settings-choice--radios",
    },
  ];
  assert.deepEqual(gather(app, index, out, CELLS, off), [kept]);
  assert.equal(existsSync(join(out, `${SHOT}-webkit-linux.png`)), false);
});

test("the stories that take no picture are read from each browser's report", () => {
  assert.deepEqual(
    unpicturedIn([
      { unpictured: [{ file: "src/a/A.stories.tsx", id: "a--one" }] },
      { unpictured: [] },
    ]),
    [{ file: "src/a/A.stories.tsx", id: "a--one" }],
  );
  assert.throws(() => unpicturedIn([{ failures: [] }]), /not a stories report/);
});
