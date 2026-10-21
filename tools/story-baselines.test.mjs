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

import { gather, problems, take } from "./story-baselines.mjs";

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
const SHOT =
  "src/settings/__screenshots__/Choice.stories.tsx/settings-choice--radios";

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
  assert.deepEqual(problems(app), []);
});

test("a baseline drawn on another platform is refused, naming it", () => {
  const app = tree({
    "src/settings/Choice.stories.tsx": "",
    [`${SHOT}-chromium-darwin.png`]: PNG,
  });
  const found = problems(app);
  assert.equal(found.length, 1);
  assert.match(found[0], /chromium-darwin\.png/);
  assert.match(found[0], /pinned image/);
});

test("a baseline whose story file is gone is refused", () => {
  const app = tree({ [`${SHOT}-webkit-linux.png`]: PNG });
  const found = problems(app);
  assert.equal(found.length, 1);
  assert.match(found[0], /Choice\.stories\.tsx/);
});

test("gather copies every baseline under its path, and nothing else", () => {
  const app = tree({
    "src/settings/Choice.stories.tsx": "story",
    [`${SHOT}-webkit-linux.png`]: PNG,
  });
  const out = mkdtempSync(join(tmpdir(), "story-baselines-out-"));
  assert.deepEqual(gather(app, out), [`${SHOT}-webkit-linux.png`]);
  assert.deepEqual(readFileSync(join(out, `${SHOT}-webkit-linux.png`)), PNG);
  assert.equal(existsSync(join(out, "src/settings/Choice.stories.tsx")), false);
});

test("take writes drawn baselines into the app", () => {
  const drawn = tree({ [`${SHOT}-chromium-linux.png`]: PNG });
  const app = tree({ "src/settings/Choice.stories.tsx": "" });
  assert.deepEqual(take(drawn, app), [`${SHOT}-chromium-linux.png`]);
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
  assert.deepEqual(take(drawn, app, { bytes: PNG.length, files: 2 }).length, 2);
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
