// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// What `runner-image.yml` and `operator-image.yml` ask the registry and the forge before they
// push, tag or sign the runner image (#2043) and the cluster operator's image (#2238). Run with
// `node --test tools/`.
//
// Every answer is given by a stand-in for the forge's API (`api`, which returns what `gh api`
// printed or throws its error) and for the registry (`get`, which returns a status and a body),
// so nothing here reaches either.
import assert from "node:assert/strict";
import { test } from "node:test";

import { anonymousPull, mainTag, packageOf, visibility } from "./image-registry.mjs";

const PACKAGE = { org: "purlis", name: "purlis-runner" };

function api(answers) {
  return (args) => {
    const path = args.at(-1);
    for (const [pattern, answer] of answers) {
      if (pattern.test(path)) {
        if (answer instanceof Error) throw answer;
        return typeof answer === "string" ? answer : JSON.stringify(answer);
      }
    }
    throw new Error(`gh api ${path}: no stand-in answer`);
  };
}

const packageAt = /^orgs\/purlis\/packages\/container\/purlis-runner$/;

test("a public package is public", () => {
  assert.equal(visibility({ ...PACKAGE, api: api([[packageAt, { visibility: "public" }]]) }), "public");
});

test("a package that is private or internal, missing, or cannot be asked about, is refused", () => {
  for (const shown of ["private", "internal", undefined]) {
    assert.throws(
      () => visibility({ ...PACKAGE, api: api([[packageAt, { visibility: shown }]]) }),
      /not public/,
      String(shown),
    );
  }
  for (const failure of ["Package not found. (HTTP 404)", "HTTP 403", "HTTP 500", "connection reset"]) {
    assert.throws(
      () => visibility({ ...PACKAGE, api: api([[packageAt, new Error(`gh api: ${failure}`)]]) }),
      /cannot tell/,
      failure,
    );
  }
  assert.throws(() => visibility({ ...PACKAGE, api: api([[packageAt, "not json"]]) }), /cannot tell/);
});

const DIGEST = `sha256:${"a".repeat(64)}`;

function registry({ token, manifest }) {
  return async (url) => {
    if (url.startsWith("https://ghcr.io/token")) return token;
    if (url.startsWith("https://ghcr.io/v2/")) return manifest;
    throw new Error(`no stand-in for ${url}`);
  };
}

test("an image the registry serves to an anonymous puller passes", async () => {
  const get = registry({ token: { status: 200, body: '{"token":"anon"}' }, manifest: { status: 200, digest: DIGEST } });
  assert.equal(await anonymousPull({ repository: "purlis/purlis-runner", digests: [DIGEST], get }), "served");
});

test("an image the registry refuses to an anonymous puller stops the run", async () => {
  for (const status of [401, 403, 404]) {
    const get = registry({ token: { status: 200, body: '{"token":"anon"}' }, manifest: { status } });
    await assert.rejects(
      anonymousPull({ repository: "purlis/purlis-runner", digests: [DIGEST], get }),
      /without signing in/,
      String(status),
    );
  }
  for (const status of [401, 403]) {
    const get = registry({ token: { status, body: "{}" }, manifest: { status: 200 } });
    await assert.rejects(
      anonymousPull({ repository: "purlis/purlis-runner", digests: [DIGEST], get }),
      /without signing in/,
      String(status),
    );
  }
});

test("one digest refused among several stops the run", async () => {
  const other = `sha256:${"b".repeat(64)}`;
  const get = async (url) => {
    if (url.startsWith("https://ghcr.io/token")) return { status: 200, body: '{"token":"anon"}' };
    return url.endsWith(other) ? { status: 404 } : { status: 200, digest: DIGEST };
  };
  await assert.rejects(
    anonymousPull({ repository: "purlis/purlis-runner", digests: [DIGEST, other], get }),
    /without signing in/,
  );
});

test("the anonymous check fails closed when the registry's answer is unclear", async () => {
  const cases = [
    { token: { status: 200, body: "{}" }, manifest: { status: 200 } },
    { token: { status: 200, body: "not json" }, manifest: { status: 200 } },
    { token: { status: 500, body: "" }, manifest: { status: 200 } },
    { token: { status: 429, body: "" }, manifest: { status: 200 } },
    { token: new Error("ECONNRESET"), manifest: { status: 200 } },
    // A 200 that does not name the digest asked for.
    { token: { status: 200, body: '{"token":"anon"}' }, manifest: { status: 200 } },
    { token: { status: 200, body: '{"token":"anon"}' }, manifest: { status: 200, digest: `sha256:${"c".repeat(64)}` } },
    // A token, and an answer that is neither a pull nor a refusal.
    { token: { status: 200, body: '{"token":"anon"}' }, manifest: { status: 500 } },
    { token: { status: 200, body: '{"token":"anon"}' }, manifest: { status: 429 } },
    { token: { status: 200, body: '{"token":"anon"}' }, manifest: new Error("ETIMEDOUT") },
  ];
  for (const { token, manifest } of cases) {
    const get = async (url) => {
      const answer = url.startsWith("https://ghcr.io/token") ? token : manifest;
      if (answer instanceof Error) throw answer;
      return answer;
    };
    await assert.rejects(
      anonymousPull({ repository: "purlis/purlis-runner", digests: [DIGEST], get }),
      /cannot show/,
      JSON.stringify({ token: String(token?.status ?? token), manifest: String(manifest?.status ?? manifest) }),
    );
  }
  await assert.rejects(anonymousPull({ repository: "purlis/purlis-runner", digests: [], get: registry({}) }), /no digest/);
});

const SHA = "1".repeat(40);
const OLDER = "2".repeat(40);
const versions = /^orgs\/purlis\/packages\/container\/purlis-runner\/versions\?per_page=100&page=(\d+)$/;

function tagged(tags) {
  return { metadata: { container: { tags } } };
}

function forge({ published, relation, tip = OLDER }) {
  return api([
    [/page=1$/, published === undefined ? [] : [tagged(["sha-" + "9".repeat(40)]), tagged(published)]],
    [versions, []],
    [/^repos\/purlis\/purlis\/compare\//, relation instanceof Error ? relation : { status: relation }],
    [/^repos\/purlis\/purlis\/commits\/main$/, { sha: tip }],
  ]);
}

function order(answers) {
  return mainTag({ ...PACKAGE, repo: "purlis/purlis", sha: SHA, api: forge(answers) });
}

test("main moves to a build of a later commit, the same commit, or main's tip", () => {
  assert.equal(order({ published: undefined }).main, true, "nothing is tagged main yet");
  assert.equal(order({ published: ["main", `sha-${OLDER}`], relation: "ahead" }).main, true);
  assert.equal(order({ published: ["main", `sha-${SHA}`] }).main, true, "a run finishing its own commit");
  assert.equal(order({ published: ["main", `sha-${OLDER}`], relation: "diverged", tip: SHA }).main, true);
});

test("main never moves back to an older commit", () => {
  const behind = order({ published: ["main", `sha-${OLDER}`], relation: "behind" });
  assert.equal(behind.main, false);
  assert.match(behind.why, /older/);
  const diverged = order({ published: ["main", `sha-${OLDER}`], relation: "diverged", tip: OLDER });
  assert.equal(diverged.main, false);
});

test("the order check fails closed when it cannot place the commit", () => {
  assert.throws(() => order({ published: ["main"] }), /names no commit/);
  assert.throws(() => order({ published: ["main", `sha-${OLDER}`], relation: "unknown" }), /says nothing/);
  assert.throws(() => order({ published: ["main", `sha-${OLDER}`], relation: new Error("gh api: HTTP 502") }), /502/);
  assert.throws(
    () => mainTag({ ...PACKAGE, repo: "purlis/purlis", sha: SHA, api: api([[versions, new Error("gh api: HTTP 403")]]) }),
    /403/,
  );
});

test("the package is the one the workflow's IMAGE names, under the repository's owner", () => {
  assert.deepEqual(packageOf({ image: "ghcr.io/purlis/purlis-runner", owner: "purlis" }), {
    org: "purlis",
    name: "purlis-runner",
  });
  assert.deepEqual(packageOf({ image: "ghcr.io/purlis/purlis-cluster-operator", owner: "Purlis" }), {
    org: "purlis",
    name: "purlis-cluster-operator",
  });
});

test("an IMAGE that is not a package of the repository's owner on ghcr.io is refused", () => {
  for (const image of [
    undefined,
    "",
    "docker.io/purlis/purlis-runner",
    "ghcr.io/someone-else/purlis-runner",
    "ghcr.io/purlis/purlis-runner:main",
    `ghcr.io/purlis/purlis-runner@sha256:${"a".repeat(64)}`,
    "ghcr.io/purlis/nested/purlis-runner",
    "ghcr.io/purlis/",
    "ghcr.io/purlis/Purlis-Runner",
  ]) {
    assert.throws(() => packageOf({ image, owner: "purlis" }), /IMAGE/, String(image));
  }
});
