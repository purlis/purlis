// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The snapshot of GitHub's ranges is made from three lists of `/meta` and no others. Run with
// `node --test tools/`.
import assert from "node:assert/strict";
import { test } from "node:test";

import { render } from "./github-ranges-refresh.mjs";

const META = {
  web: ["140.82.112.0/20", "4.237.22.38/32"],
  api: ["4.237.22.34/32", "140.82.112.0/20"],
  git: ["2A0A:A440::/29"],
  actions: ["20.20.20.0/24"],
  pages: ["185.199.108.0/22"],
  hooks: ["192.30.252.0/22"],
};

test("the snapshot holds the web, api and git ranges, sorted and without repeats", () => {
  const lines = render(META)
    .split("\n")
    .filter((line) => line && !line.startsWith("#"));
  assert.deepEqual(lines, [
    "140.82.112.0/20",
    "2a0a:a440::/29",
    "4.237.22.34/32",
    "4.237.22.38/32",
  ]);
});

test("the snapshot holds no list that carries other people's sites or webhook sources", () => {
  const text = render(META);
  for (const range of ["20.20.20.0/24", "185.199.108.0/22", "192.30.252.0/22"]) {
    assert.ok(!text.includes(range), range);
  }
});

test("an answer without one of the three lists, or with a line that is no CIDR, is refused", () => {
  assert.throws(() => render({ ...META, git: [] }), /no "git" list/);
  assert.throws(() => render({ ...META, web: ["not-a-range"] }), /not a CIDR/);
});
