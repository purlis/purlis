// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The dev channel only moves forward. Run with `node --test tools/`.
import assert from "node:assert/strict";
import { test } from "node:test";

import { ask, decide, newerVersion, publishedBuild } from "./dev-build-order.mjs";

const OLD = "4c7854ee".padEnd(40, "0");
const NEW = "e827217c".padEnd(40, "1");

/** `dev.json` as `release.yml`'s `manifest` job writes it for `version`, built from `sha`. */
function manifest(version, sha) {
  return JSON.stringify({
    version,
    notes: `charter ${version}, a dev build of ${sha}.`,
    pub_date: "2026-10-13T14:09:00Z",
    platforms: {},
  });
}

test("the published build is read from dev.json: its version and the commit its notes name", () => {
  assert.deepEqual(publishedBuild(manifest("0.4.2-dev.653", NEW)), {
    version: "0.4.2-dev.653",
    sha: NEW,
  });
});

test("a dev.json that names no commit is refused, never read as nothing published", () => {
  assert.throws(
    () => publishedBuild(JSON.stringify({ version: "0.4.2-dev.653", notes: "a dev build." })),
    /names no commit/,
  );
  assert.throws(() => publishedBuild(JSON.stringify({ notes: `of ${NEW}` })), /no version/);
  assert.throws(() => publishedBuild("not json"), /not JSON/);
});

test("a version is newer by its numbers, and a dev build is older than the release it becomes", () => {
  assert.equal(newerVersion("0.4.2-dev.654", "0.4.2-dev.653"), true);
  assert.equal(newerVersion("0.4.2-dev.1000", "0.4.2-dev.999"), true);
  assert.equal(newerVersion("0.4.2-dev.653", "0.4.2-dev.653"), false);
  assert.equal(newerVersion("0.4.2-dev.653", "0.4.2-dev.654"), false);
  assert.equal(newerVersion("0.4.3-dev.700", "0.4.2-dev.999"), true);
  assert.equal(newerVersion("0.10.0-dev.700", "0.9.9-dev.999"), true);
  assert.equal(newerVersion("0.4.2-dev.700", "0.4.2"), false);
  assert.equal(newerVersion("0.4.2", "0.4.2-dev.700"), true);
  assert.throws(() => newerVersion("0.4.2-dev.x", "0.4.2-dev.1"), /not a version/);
});

test("the first dev build publishes: nothing is there to go back from", () => {
  const order = decide({ published: null, version: "0.4.2-dev.1", sha: NEW, relation: null });
  assert.equal(order.publish, true);
});

test("a build of a commit after the published one publishes", () => {
  const order = decide({
    published: { version: "0.4.2-dev.653", sha: OLD },
    version: "0.4.2-dev.654",
    sha: NEW,
    relation: "ahead",
  });
  assert.equal(order.publish, true);
});

test("a build of a commit before the published one publishes nothing (run 654 over 653, 2026-10-13)", () => {
  const order = decide({
    published: { version: "0.4.2-dev.653", sha: NEW },
    version: "0.4.2-dev.654",
    sha: OLD,
    relation: "behind",
  });
  assert.equal(order.publish, false);
  assert.match(order.why, /older/);
  assert.match(order.why, new RegExp(NEW));
});

test("a second run for the commit already published publishes nothing", () => {
  const order = decide({
    published: { version: "0.4.2-dev.653", sha: NEW },
    version: "0.4.2-dev.660",
    sha: NEW,
    relation: "identical",
  });
  assert.equal(order.publish, false);
});

test("the same run started again finishes its own publish", () => {
  // `dev.json` goes up before its weekly twin and the release's note, so a run that died
  // between them has to be able to finish.
  const order = decide({
    published: { version: "0.4.2-dev.653", sha: NEW },
    version: "0.4.2-dev.653",
    sha: NEW,
    relation: "identical",
  });
  assert.equal(order.publish, true);
});

test("a newer commit under a number that is not newer publishes nothing: no app would take it", () => {
  // An old run started again after a later run published an older commit.
  const order = decide({
    published: { version: "0.4.2-dev.654", sha: OLD },
    version: "0.4.2-dev.653",
    sha: NEW,
    relation: "ahead",
  });
  assert.equal(order.publish, false);
  assert.match(order.why, /0\.4\.2-dev\.654/);
});

test("main rewritten under the published build still publishes main", () => {
  const order = decide({
    published: { version: "0.4.2-dev.653", sha: OLD },
    version: "0.4.2-dev.654",
    sha: NEW,
    relation: "diverged",
  });
  assert.equal(order.publish, true);
  assert.match(order.why, /not in this commit's history/);
});

test("an answer the forge did not give is refused", () => {
  assert.throws(
    () =>
      decide({
        published: { version: "0.4.2-dev.653", sha: OLD },
        version: "0.4.2-dev.654",
        sha: NEW,
        relation: "",
      }),
    /compare/,
  );
});

/** A forge that answers `ask`'s three reads from `assets`, `text` and `status`. */
function forge({ release = true, assets = [], text = "", status = "ahead" }) {
  const calls = [];
  const api = (args) => {
    calls.push(args.join(" "));
    const path = args.at(-1);
    if (path.endsWith("/releases/tags/dev")) {
      if (!release) throw Object.assign(new Error("gh: Not Found (HTTP 404)"), { status: 1 });
      return JSON.stringify({ assets });
    }
    if (path.includes("/releases/assets/")) return text;
    if (path.includes("/compare/")) return JSON.stringify({ status });
    throw new Error(`unexpected read: ${path}`);
  };
  return { api, calls };
}

test("asking reads the dev release, its dev.json, and how the two commits stand", () => {
  const { api, calls } = forge({
    assets: [
      { id: 7, name: "dev-weekly.json" },
      { id: 9, name: "dev.json" },
    ],
    text: manifest("0.4.2-dev.653", NEW),
    status: "behind",
  });
  const order = ask({ repo: "purlis/purlis", version: "0.4.2-dev.654", sha: OLD, api });
  assert.equal(order.publish, false);
  assert.deepEqual(calls, [
    "repos/purlis/purlis/releases/tags/dev",
    "-H Accept: application/octet-stream repos/purlis/purlis/releases/assets/9",
    `repos/purlis/purlis/compare/${NEW}...${OLD}`,
  ]);
});

test("asking with no dev release, or none with a dev.json, publishes and compares nothing", () => {
  for (const given of [{ release: false }, { assets: [{ id: 1, name: "purlis.intoto.jsonl" }] }]) {
    const { api, calls } = forge(given);
    const order = ask({ repo: "purlis/purlis", version: "0.4.2-dev.1", sha: NEW, api });
    assert.equal(order.publish, true);
    assert.equal(calls.length, 1);
  }
});

test("asking for the commit already published compares nothing", () => {
  const { api, calls } = forge({
    assets: [{ id: 9, name: "dev.json" }],
    text: manifest("0.4.2-dev.653", NEW),
  });
  const order = ask({ repo: "purlis/purlis", version: "0.4.2-dev.660", sha: NEW, api });
  assert.equal(order.publish, false);
  assert.equal(calls.length, 2);
});

test("a forge that fails for any reason but a missing release fails the asking", () => {
  const api = () => {
    throw new Error("gh: Bad Gateway (HTTP 502)");
  };
  assert.throws(
    () => ask({ repo: "purlis/purlis", version: "0.4.2-dev.1", sha: NEW, api }),
    /502/,
  );
});
