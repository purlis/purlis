// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The stories image's own checks (#2942, #2956). Run with `node --test tools/`.
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import {
  DRAWS,
  PIN,
  imageProblems,
  lockedPlaywright,
  pinnedImage,
} from "./story-image.mjs";

const DIGEST = `sha256:${"a1".repeat(32)}`;
const OTHER = `sha256:${"b2".repeat(32)}`;

const pinned = (version = "1.64.1", digest = DIGEST) =>
  `mcr.microsoft.com/playwright:v${version}-noble@${digest}`;

const dockerfile = (image = pinned()) => `# Copyright 2026 Purlis
# FROM mcr.microsoft.com/playwright:v0.0.0-noble in a comment is not the pin.
FROM ${image}
`;

/** The workflow that draws, reading the pin. */
const draws = {
  name: DRAWS,
  text: `jobs:
  shard:
    # The image: mcr.microsoft.com/playwright:v0.0.0-noble in a comment is not a use.
    container:
      image: \${{ needs.changes.outputs.image }}
      options: --init --ipc=host
`,
};

/** Another workflow with one job, whose lines are `body`. */
const other = (body) => ({ name: "other.yml", text: `jobs:\n  a:\n${body}\n` });

test("a pin of the locked version and a workflow that reads it pass", () => {
  assert.deepEqual(imageProblems("1.64.1", dockerfile(), [draws]), []);
});

test("a pin by its tag alone is refused", () => {
  const found = imageProblems(
    "1.64.1",
    dockerfile("mcr.microsoft.com/playwright:v1.64.1-noble"),
    [draws],
  );
  assert.equal(found.length, 1);
  assert.match(found[0], /digest/);
});

test("a pin of another Playwright than the lockfile's is refused, saying what to name", () => {
  const found = imageProblems("1.65.0", dockerfile(), [draws]);
  assert.equal(found.length, 1);
  assert.match(found[0], /locks Playwright 1\.65\.0/);
  assert.match(found[0], /v1\.65\.0-noble@sha256:/);
});

test("a pin with no FROM, or two, is refused", () => {
  assert.match(imageProblems("1.64.1", "# nothing\n", [draws])[0], /one FROM/);
  assert.match(
    imageProblems("1.64.1", `FROM ${pinned()}\nFROM ${pinned()}\n`, [draws])[0],
    /one FROM/,
  );
});

test("the workflow that draws, naming no pinned image, is refused", () => {
  const found = imageProblems("1.64.1", dockerfile(), [
    { name: DRAWS, text: "jobs: {}\n" },
  ]);
  assert.equal(found.length, 1);
  assert.match(found[0], /does not draw in the pinned image/);
});

test("the workflow that draws, naming a literal image, is refused both ways", () => {
  const found = imageProblems("1.64.1", dockerfile(), [
    { name: DRAWS, text: `    container:\n      image: ${pinned("1.64.1", OTHER)}\n` },
  ]);
  assert.equal(found.length, 2);
  assert.match(found[1], new RegExp(OTHER));
});

// Every way a workflow can name a container, in any workflow file (#2942's review).
for (const [form, line] of [
  ["a mapping's image", `    container:\n      image: IMAGE`],
  ["a quoted image", `    container:\n      image: "IMAGE"`],
  ["a plain string", `    container: IMAGE`],
  ["a flow mapping", `    container: { image: IMAGE, options: --init }`],
  ["a step's docker://", `    steps:\n      - uses: docker://IMAGE`],
  ["a service", `    services:\n      a:\n        image: IMAGE # why`],
]) {
  test(`${form} in any workflow must be the pinned image`, () => {
    const named = (image) => other(line.replace("IMAGE", image));
    assert.deepEqual(
      imageProblems("1.64.1", dockerfile(), [draws, named(pinned())]),
      [],
    );
    for (const wrong of [
      pinned("1.64.1", OTHER),
      "mcr.microsoft.com/playwright:v1.64.1-noble",
      "mcr.microsoft.com/playwright",
    ]) {
      const found = imageProblems("1.64.1", dockerfile(), [draws, named(wrong)]);
      assert.equal(found.length, 1, wrong);
      assert.match(found[0], /other\.yml:\d+/);
      assert.ok(found[0].includes(wrong));
    }
  });
}

test("a comment that names an image is no use of it", () => {
  assert.deepEqual(
    imageProblems("1.64.1", dockerfile(), [
      draws,
      other("    # mcr.microsoft.com/playwright:v0.0.0-noble\n    x: y # mcr.microsoft.com/playwright"),
    ]),
    [],
  );
});

test("the pin's FROM is read, a comment's is not", () => {
  assert.equal(pinnedImage(dockerfile()), pinned());
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

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");

test("the repository's own pin and every workflow pass", () => {
  const locked = lockedPlaywright(JSON.parse(read("app/package-lock.json")));
  assert.deepEqual(
    imageProblems(
      locked,
      read(PIN),
      readdirSync(join(root, ".github/workflows"))
        .filter((file) => /\.ya?ml$/.test(file))
        .map((file) => `.github/workflows/${file}`)
        .map((name) => ({ name, text: read(name) })),
    ),
    [],
  );
});

// One pull request moves the packages and the image: each half is a block of one group, and the
// ordinary npm block leaves these packages to it. The blocks of a directory must stay distinct.
test("Dependabot's playwright group moves the packages and the pin together", () => {
  const config = read(".github/dependabot.yml");
  const blocks = config
    .split(/\n(?=  - package-ecosystem:)/)
    .filter((block) => /multi-ecosystem-group: playwright\b/.test(block));
  assert.equal(blocks.length, 2);
  const docker = blocks.find((block) => /package-ecosystem: docker/.test(block));
  const npm = blocks.find((block) => /package-ecosystem: npm/.test(block));
  assert.ok(docker.includes(`directory: /${dirname(PIN)}`));
  const names = ["playwright", "@playwright/*", "vitest", "@vitest/*"];
  const web = config
    .split(/\n(?=  - package-ecosystem:)/)
    .find((block) => /groups:\s+web:/.test(block));
  for (const name of names) {
    assert.ok(npm.includes(`"${name}"`), `the group names ${name}`);
    assert.match(web, new RegExp(`dependency-name: "?${name.replace("*", "\\*")}"?\\n`), `web ignores ${name}`);
  }
});
