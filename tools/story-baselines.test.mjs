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
import { join } from "node:path";
import { test } from "node:test";

import { gather, problems, take } from "./story-baselines.mjs";

const PNG = Buffer.from([
  0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0,
]);
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
