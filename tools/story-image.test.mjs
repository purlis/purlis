// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The stories image's own checks (#2942). Run with `node --test tools/`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { WORKFLOWS, imageProblems, lockedPlaywright } from "./story-image.mjs";

const DIGEST = `sha256:${"a1".repeat(32)}`;
const OTHER = `sha256:${"b2".repeat(32)}`;

/** A workflow whose job runs in `image`. */
const workflow = (image) => `jobs:
  stories:
    # The image: mcr.microsoft.com/playwright:v0.0.0-noble in a comment is not a use.
    container:
      image: ${image}
      options: --init --ipc=host
`;

const pinned = (version = "1.64.1", digest = DIGEST) =>
  `mcr.microsoft.com/playwright:v${version}-noble@${digest}`;

test("both workflows naming the locked version's image by tag and digest pass", () => {
  assert.deepEqual(
    imageProblems("1.64.1", [
      { name: "stories.yml", text: workflow(pinned()) },
      { name: "stories-baselines.yml", text: workflow(pinned()) },
    ]),
    [],
  );
});

test("an image named by its tag alone is refused", () => {
  const found = imageProblems("1.64.1", [
    { name: "stories.yml", text: workflow(pinned()) },
    {
      name: "stories-baselines.yml",
      text: workflow("mcr.microsoft.com/playwright:v1.64.1-noble"),
    },
  ]);
  assert.equal(found.length, 1);
  assert.match(found[0], /stories-baselines\.yml:5/);
  assert.match(found[0], /digest/);
});

test("a digest that differs from the other workflow's is refused, naming both", () => {
  const found = imageProblems("1.64.1", [
    { name: "stories.yml", text: workflow(pinned()) },
    { name: "stories-baselines.yml", text: workflow(pinned("1.64.1", OTHER)) },
  ]);
  assert.equal(found.length, 1);
  assert.match(found[0], new RegExp(OTHER));
  assert.match(found[0], new RegExp(DIGEST));
});

test("an image of another Playwright than the lockfile's is refused", () => {
  const found = imageProblems("1.65.0", [
    { name: "stories.yml", text: workflow(pinned()) },
    { name: "stories-baselines.yml", text: workflow(pinned()) },
  ]);
  assert.equal(found.length, 2);
  assert.match(found[0], /locks Playwright 1\.65\.0/);
  assert.match(found[0], /v1\.65\.0-noble@sha256:/);
});

test("a workflow that names no Playwright image is refused", () => {
  const found = imageProblems("1.64.1", [
    { name: "stories.yml", text: workflow(pinned()) },
    { name: "stories-baselines.yml", text: "jobs: {}\n" },
  ]);
  assert.equal(found.length, 1);
  assert.match(found[0], /stories-baselines\.yml names no Playwright image/);
});

test("the lockfile's Playwright is read from app/package-lock.json", () => {
  assert.equal(
    lockedPlaywright({
      packages: { "node_modules/playwright": { version: "1.64.1" } },
    }),
    "1.64.1",
  );
  assert.throws(() => lockedPlaywright({ packages: {} }), /no Playwright/);
});

test("the repository's own stories workflows pass", () => {
  const root = join(dirname(fileURLToPath(import.meta.url)), "..");
  const read = (path) => readFileSync(join(root, path), "utf8");
  const locked = lockedPlaywright(JSON.parse(read("app/package-lock.json")));
  assert.deepEqual(
    imageProblems(
      locked,
      WORKFLOWS.map((path) => ({ name: path, text: read(path) })),
    ),
    [],
  );
});
