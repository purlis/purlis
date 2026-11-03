// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Which untagged versions `image-leftovers.yml` (#4930) lists as safe to delete, read from recorded
// answers (`tools/image-leftovers-fixtures/`, recorded 2026-11-03): the runner package's tags and
// the manifests they reach, read anonymously from the registry; release runs and one run's
// artifacts from the Actions API; and the two digests that run recorded. The packages API's list
// of versions could not be recorded (the recording token had no `read:packages`), so `versionsOf`
// builds it from the registry's graph in the shape that API documents.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  BOUND_MS,
  CAP,
  FAILED,
  couldPublish,
  graph,
  isPlainImage,
  leftovers,
  pages,
  reachable,
  registry,
  removable,
  report,
  runDigests,
} from "./image-leftovers.mjs";

const fixture = (name) =>
  JSON.parse(readFileSync(new URL(`./image-leftovers-fixtures/${name}`, import.meta.url), "utf8"));
const recordedRegistry = fixture("registry-runner.json");
const releaseRuns = fixture("release-runs.json").workflow_runs;
const recordedArtifacts = fixture("artifacts-41581143090.json").artifacts;
const recorded = fixture("digests-41581143090.json");

const REPO = "purlis/purlis";
const WORKFLOW = "release.yml";
const DAY = 24 * 60 * 60 * 1000;
const NOW = Date.parse("2026-11-08T12:00:00Z");
const LEFTOVER = `sha256:${"e".repeat(64)}`;
const OTHER = `sha256:${"f".repeat(64)}`;
const INDEX = "application/vnd.oci.image.index.v1+json";
const IMAGE = "application/vnd.oci.image.manifest.v1+json";

/** The packages API's list of versions: one per tagged digest and per manifest a tag reaches. */
function versionsOf({ tags, manifests }, extra = []) {
  const byDigest = new Map();
  for (const [tag, digest] of Object.entries(tags)) byDigest.set(digest, [...(byDigest.get(digest) ?? []), tag]);
  for (const manifest of Object.values(manifests)) {
    for (const child of manifest.manifests ?? []) if (!byDigest.has(child.digest)) byDigest.set(child.digest, []);
  }
  let id = 600_000_000;
  const listed = [...byDigest].map(([name, tagged]) => ({
    id: (id += 1),
    name,
    created_at: "2026-11-03T05:08:30Z",
    updated_at: "2026-11-03T05:08:30Z",
    metadata: { package_type: "container", container: { tags: tagged } },
  }));
  return [...listed, ...extra];
}

const untagged = (name, created_at, id = 700_000_001) => ({
  id,
  name,
  created_at,
  updated_at: created_at,
  metadata: { package_type: "container", container: { tags: [] } },
});

// The recorded cancelled release, 41577697984, as if it had pushed one architecture's image before
// it stopped: the leftover #4930 is about.
const cancelled = releaseRuns.find((run) => run.id === 41577697984);
const failedRun = (overrides = {}) => ({ ...cancelled, digests: [LEFTOVER], ...overrides });
const plain = { mediaType: IMAGE, config: { mediaType: "application/vnd.oci.image.config.v1+json" } };
const done = releaseRuns.filter((run) => run.status === "completed" && run.id !== cancelled.id);

function select(overrides = {}) {
  return leftovers({
    versions: versionsOf(recordedRegistry, [untagged(LEFTOVER, "2026-11-03T04:40:00Z")]),
    kept: reachable(recordedRegistry),
    runs: [...done, failedRun()],
    repo: REPO,
    workflow: WORKFLOW,
    now: NOW,
    ...overrides,
  });
}

// ---- Condition 1: nothing a tag reaches ----

test("every manifest a tag reaches is kept: the indexes' own images, signatures and attestations", () => {
  const kept = reachable(recordedRegistry);
  // The architecture images of the recorded release, which no tag names but its index holds.
  assert.ok(kept.has(recorded.amd64) && kept.has(recorded.arm64));
  // Its attestation and signature manifests, which only the `sha256-<digest>` index holds.
  assert.ok(kept.has("sha256:1b3fe7493ce02fd2eed8269310c9df18e53b01b621181fed025fa115ec1b6fa9"));
  assert.ok(kept.has("sha256:272d2d4186eaeef0d28177512d8599cd23c5a98a45924ecf5dfa4c89e7acd0a7"));
  // `main`'s own images.
  assert.ok(kept.has("sha256:d393ae758105fcb3d4f0df64935e4d0a59872470c996766f3135f338eb3b2fea"));
  assert.ok(!kept.has(LEFTOVER));
});

test("a referrers tag's subject is kept, and so is everything under it when no other tag holds it", () => {
  const signatures = "sha256:4a0bd4176b31f166f30346eb7ad8cfeb4fe4610540e3e468a1d36ed47d5a661f";
  assert.ok(reachable({ tags: { [`sha256-${"e".repeat(64)}`]: signatures }, manifests: recordedRegistry.manifests }).has(LEFTOVER));
  assert.ok(reachable({ tags: { [`sha256-${"e".repeat(64)}.sig`]: signatures }, manifests: recordedRegistry.manifests }).has(LEFTOVER));
  // A signed index whose `sha-<commit>` tag moved to another index: only its signature's tag
  // holds it now, and its two images must stay with it.
  const signed = "sha256:8237925ac22d406590b1daae0c27398e1aba44024b148a00264123fbcbf1dbed";
  const tags = { [`sha256-${signed.slice(7)}`]: signatures };
  const kept = reachable({ tags, manifests: recordedRegistry.manifests });
  assert.ok(kept.has(signed));
  assert.ok(kept.has("sha256:aa446a16e363def3509006924a51b04ce76b93f038309ad9b25b6aab897971bc"));
  assert.ok(kept.has("sha256:a4de49e7d983fb6738eddc8fbb6f66dcbbd483c0d9bc5e3bd4a5afd24740d17d"));
});

test("the graph cannot be told when a tagged manifest or an index it holds was not read", () => {
  assert.throws(() => reachable({ tags: { main: LEFTOVER }, manifests: {} }), /cannot tell what main reaches: its manifest .* was not read/);
  const holdsAnIndex = { [OTHER]: { mediaType: INDEX, manifests: [{ digest: LEFTOVER, mediaType: INDEX }] } };
  assert.throws(() => reachable({ tags: { main: OTHER }, manifests: holdsAnIndex }), /was not read/);
});

// ---- Condition 2: recorded by a failed run that could publish ----

test("only this repository's own run of the publishing workflow, from main or a v* tag, could publish", () => {
  const ok = { ...cancelled };
  assert.ok(couldPublish(ok, { repo: REPO, workflow: WORKFLOW }));
  assert.ok(couldPublish({ ...ok, event: "push", head_branch: "v0.5.0" }, { repo: REPO, workflow: WORKFLOW }));
  const operator = { ...ok, path: ".github/workflows/operator-image.yml" };
  assert.ok(couldPublish(operator, { repo: REPO, workflow: "operator-image.yml" }));
  for (const [what, run, workflow] of [
    ["a fork's run", { ...ok, head_repository: { full_name: "someone/purlis" } }, WORKFLOW],
    ["a pull request's run", { ...ok, event: "pull_request" }, WORKFLOW],
    ["a dispatch", { ...ok, event: "workflow_dispatch" }, WORKFLOW],
    ["the nightly profile guard", { ...ok, event: "schedule" }, WORKFLOW],
    ["another branch", { ...ok, head_branch: "feature" }, WORKFLOW],
    ["another workflow's run", { ...ok, path: ".github/workflows/ci.yml" }, WORKFLOW],
    ["a tag that is not a version", { ...ok, event: "push", head_branch: "vandal" }, WORKFLOW],
    ["a pushed tag for the operator", { ...operator, event: "push", head_branch: "v0.5.0" }, "operator-image.yml"],
  ]) {
    assert.ok(!couldPublish(run, { repo: REPO, workflow }), what);
    assert.deepEqual(select({ runs: [...done, failedRun(run)] }).candidates, [], what);
  }
});

test("an untagged image a failed run recorded, that nothing reaches, is a candidate three days after the run", () => {
  const { candidates, keep } = select();
  assert.deepEqual(candidates, [{ id: 700_000_001, digest: LEFTOVER, run: cancelled.id }]);
  assert.deepEqual(keep, []);
});

test("the recorded release's own images are never candidates: a tag reaches them", () => {
  const run = { ...cancelled, digests: [recorded.amd64, recorded.arm64] };
  const versions = versionsOf(recordedRegistry).map((v) => (v.name === recorded.amd64 ? { ...v, created_at: "2026-11-03T04:40:00Z" } : v));
  assert.deepEqual(select({ versions, runs: [run] }).candidates, []);
});

test("an untagged version no failed run recorded is kept", () => {
  const { candidates, keep } = select({ runs: done });
  assert.deepEqual(candidates, []);
  assert.deepEqual(keep, [{ digest: LEFTOVER, why: "no failed publishing run recorded it" }]);
});

test("a version a run that did not fail recorded too is kept: a rerun under way, or one that passed", () => {
  for (const other of [releaseRuns.find((run) => run.conclusion === "success"), releaseRuns.find((run) => run.status === "in_progress")]) {
    const { candidates, keep } = select({ runs: [failedRun(), { ...other, digests: [LEFTOVER] }] });
    assert.deepEqual(candidates, []);
    assert.match(keep[0].why, new RegExp(`run ${other.id}, which did not fail, recorded it`));
  }
});

test("only a run that failed, was cancelled or timed out counts as failed", () => {
  assert.deepEqual([...FAILED].sort(), ["cancelled", "failure", "timed_out"]);
  for (const conclusion of ["success", "skipped", "neutral", "action_required", "stale", null]) {
    assert.deepEqual(select({ runs: [failedRun({ conclusion })] }).candidates, [], String(conclusion));
  }
});

// ---- Condition 3: pushed while the run ran, three days on ----

test("a run that ended less than three days ago keeps what it recorded, so a rerun can still use it", () => {
  assert.equal(BOUND_MS, 3 * DAY);
  const ended = Date.parse(cancelled.updated_at);
  assert.deepEqual(select({ now: ended + 3 * DAY - 1 }).candidates, []);
  assert.match(select({ now: ended + 3 * DAY - 1 }).keep[0].why, /ended less than 3 days ago/);
  assert.equal(select({ now: ended + 3 * DAY }).candidates.length, 1);
});

test("a version not pushed while its run ran is kept; the run's window opens at its creation", () => {
  for (const created of ["2026-11-03T04:21:59Z", "2026-11-03T04:58:23Z"]) {
    const { candidates, keep } = select({ versions: versionsOf(recordedRegistry, [untagged(LEFTOVER, created)]) });
    assert.deepEqual(candidates, [], created);
    assert.match(keep[0].why, /was not pushed while run 41577697984 ran/);
  }
  // A rerun moves `run_started_at`; an earlier attempt's push still falls in the run.
  const rerun = failedRun({ run_attempt: 2, run_started_at: "2026-11-03T04:50:00Z" });
  assert.equal(select({ runs: [...done, rerun] }).candidates.length, 1);
});

// ---- Conditions 4 and 5: a plain image nothing refers to, and few of them ----

const candidate = { id: 700_000_001, digest: LEFTOVER, run: cancelled.id };

test("only a plain image manifest is listed: an index, a referrer or an artifact is kept", () => {
  const index = { mediaType: INDEX, manifests: [] };
  const referrer = recordedRegistry.manifests["sha256:6680ef18840340309996dae95cafc0662cf30b3f8c20b818691c457a06cd15cc"];
  const artifact = { ...plain, artifactType: "application/vnd.example" };
  const docker = { mediaType: "application/vnd.docker.distribution.manifest.v2+json", config: { mediaType: "application/vnd.docker.container.image.v1+json" } };
  assert.ok(isPlainImage(plain) && isPlainImage(docker));
  assert.ok(isPlainImage(recordedRegistry.manifests[recorded.amd64]), "the recorded architecture image is one");
  assert.deepEqual(removable({ candidates: [candidate], manifests: { [LEFTOVER]: plain }, referrers: { [LEFTOVER]: [] } }).removable, [candidate]);
  for (const manifest of [index, referrer, artifact, undefined, null, {}]) {
    assert.ok(!isPlainImage(manifest), JSON.stringify(manifest));
    const { removable: listed, keep } = removable({ candidates: [candidate], manifests: { [LEFTOVER]: manifest }, referrers: { [LEFTOVER]: [] } });
    assert.deepEqual(listed, []);
    assert.match(keep[0].why, /is not a plain image manifest/);
  }
});

test("a candidate the referrers API lists anything for is kept, and one whose referrers were not read stops the report", () => {
  const signature = { digest: OTHER, mediaType: IMAGE, artifactType: "application/vnd.dev.sigstore.bundle.v0.3+json" };
  const { removable: listed, keep } = removable({ candidates: [candidate], manifests: { [LEFTOVER]: plain }, referrers: { [LEFTOVER]: [signature] } });
  assert.deepEqual(listed, []);
  assert.deepEqual(keep, [{ digest: LEFTOVER, why: "1 manifests refer to it" }]);
  assert.throws(() => removable({ candidates: [candidate], manifests: { [LEFTOVER]: plain }, referrers: {} }), /what refers to .* was not read/);
});

test("more candidates than the cap lists none and says so", () => {
  assert.equal(CAP, 4);
  const many = Array.from({ length: CAP + 1 }, (_, i) => ({ id: i, digest: `sha256:${String(i).repeat(64)}`, run: 1 }));
  assert.throws(
    () =>
      removable({
        candidates: many,
        manifests: Object.fromEntries(many.map((c) => [c.digest, plain])),
        referrers: Object.fromEntries(many.map((c) => [c.digest, []])),
      }),
    /5 versions qualify, more than the 4 a failed run could leave: none is listed/,
  );
});

test("an answer that cannot be read stops the selection", () => {
  assert.throws(() => select({ versions: "nope" }), /the list of versions is not a list/);
  assert.throws(() => select({ versions: [untagged(LEFTOVER, "yesterday")] }), /created_at/);
  assert.throws(() => select({ versions: [untagged("latest", "2026-11-03T04:40:00Z")] }), /not a sha256 digest/);
  assert.throws(() => select({ runs: [failedRun({ updated_at: "soon" })] }), /updated_at/);
  assert.throws(() => select({ runs: [failedRun({ digests: ["sha256:abc"] })] }), /not a sha256 digest/);
  assert.throws(() => select({ now: Number.NaN }), /now/);
});

test("a run's digests are read from its digest-<arch> artifacts, by name, and nothing else", () => {
  const read = [];
  const digests = runDigests({
    artifacts: recordedArtifacts,
    download: (artifact) => {
      read.push(artifact.name);
      return `${recorded[artifact.name.slice("digest-".length)]}\n`;
    },
  });
  assert.deepEqual(read.sort(), ["digest-amd64", "digest-arm64"]);
  assert.deepEqual(digests.sort(), [recorded.amd64, recorded.arm64].sort());
  const amd64 = recordedArtifacts.find((a) => a.name === "digest-amd64");
  assert.throws(() => runDigests({ artifacts: [amd64], download: () => "sha256:abc" }), /digest-amd64 holds no sha256 digest/);
  // An expired artifact cannot be read, and says nothing about what the run pushed.
  const expired = recordedArtifacts.map((a) => ({ ...a, expired: true }));
  assert.deepEqual(runDigests({ artifacts: expired, download: () => assert.fail("read an expired artifact") }), []);
});

// ---- What the forge and the registry answer ----

test("a paged list is read to its end, and one that holds fewer than its total_count stops the report", () => {
  const hundred = Array.from({ length: 100 }, (_, i) => ({ id: i }));
  const asked = [];
  const api = (path) => {
    asked.push(path);
    return path.endsWith("page=1") ? { total_count: 130, items: hundred } : { total_count: 130, items: hundred.slice(0, 30) };
  };
  assert.equal(pages(api, "x?created=>=2026-10-31", "items").length, 130);
  assert.deepEqual(asked, ["x?created=>=2026-10-31&per_page=100&page=1", "x?created=>=2026-10-31&per_page=100&page=2"]);
  // The Actions API stops at 1,000 runs.
  assert.throws(() => pages(() => ({ total_count: 1200, items: [] }), "x", "items"), /listed 0 of 1200/);
  assert.throws(() => pages(() => ({ message: "Not Found" }), "x", "items"), /did not answer a list/);
});

/** A registry that serves the recorded graph, its tags in pages of `per` linked as ghcr links them. */
function fakeRegistry({ tags = recordedRegistry.tags, manifests = recordedRegistry.manifests, per = 4, referrers = () => ({ status: 404, body: '{"errors":[]}' }) } = {}) {
  const names = Object.keys(tags);
  const asked = [];
  const get = async (url, headers) => {
    asked.push(url);
    if (url.startsWith("https://ghcr.io/token?")) return { status: 200, body: JSON.stringify({ token: "anonymous" }) };
    assert.equal(headers.Authorization, "Bearer anonymous");
    const list = /\/tags\/list\?n=1000(?:&last=(.+))?$/.exec(url);
    if (list) {
      const from = list[1] ? names.indexOf(list[1]) + 1 : 0;
      const page = names.slice(from, from + per);
      const more = from + per < names.length;
      return { status: 200, link: more ? `</v2/purlis/purlis-runner/tags/list?n=1000&last=${page.at(-1)}>; rel="next"` : null, body: JSON.stringify({ tags: page }) };
    }
    const manifest = /\/manifests\/(.+)$/.exec(url)?.[1];
    if (manifest) {
      const digest = tags[manifest] ?? manifest;
      return digest in manifests ? { status: 200, digest, body: JSON.stringify(manifests[digest]) } : { status: 404, body: '{"errors":[]}' };
    }
    const referred = /\/referrers\/(.+)$/.exec(url)?.[1];
    if (referred) return referrers(referred);
    throw new Error(`unexpected ${url}`);
  };
  return { get, asked };
}

test("the registry's tags are read across its pages, and the graph holds every tag and what it reaches", async () => {
  const { get, asked } = fakeRegistry();
  const read = await graph(await registry("purlis/purlis-runner", get));
  assert.deepEqual(read.tags, recordedRegistry.tags);
  assert.equal(asked.filter((u) => u.includes("/tags/list")).length, 3);
  assert.deepEqual(reachable(read), reachable(recordedRegistry));
});

test("a subject the registry no longer has is kept and walks to nothing; any other refusal stops the report", async () => {
  const gone = `sha256-${"e".repeat(64)}`;
  const tags = { ...recordedRegistry.tags, [gone]: "sha256:4a0bd4176b31f166f30346eb7ad8cfeb4fe4610540e3e468a1d36ed47d5a661f" };
  assert.ok(reachable(await graph(await registry("purlis/purlis-runner", fakeRegistry({ tags }).get))).has(LEFTOVER));
  const { get } = fakeRegistry();
  const refusing = async (url, headers) => (url.includes("/manifests/sha-") ? { status: 401, body: "" } : get(url, headers));
  await assert.rejects(graph(await registry("purlis/purlis-runner", refusing)), /answered 401/);
  const lying = async (url, headers) => {
    const answer = await get(url, headers);
    return url.endsWith(recorded.amd64) ? { ...answer, digest: OTHER } : answer;
  };
  await assert.rejects((await registry("purlis/purlis-runner", lying)).manifest(recorded.amd64), /was answered with/);
});

test("the referrers API: ghcr's 404 means its referrers tags hold them; a list is read; anything else stops", async () => {
  const signature = { digest: OTHER, mediaType: IMAGE };
  const ask = (answer) => registry("purlis/purlis-runner", fakeRegistry({ referrers: () => answer }).get);
  assert.deepEqual(await (await ask({ status: 404, body: "" })).referrers(LEFTOVER), []);
  assert.deepEqual(await (await ask({ status: 200, body: JSON.stringify({ manifests: [signature] }) })).referrers(LEFTOVER), [signature]);
  await assert.rejects((await ask({ status: 500, body: "" })).referrers(LEFTOVER), /answered 500/);
  await assert.rejects((await ask({ status: 200, body: "{}" })).referrers(LEFTOVER), /listed no manifests/);
});

test("the report, end to end on the recorded answers: one leftover listed, the release's images kept", async () => {
  const manifests = { ...recordedRegistry.manifests, [LEFTOVER]: plain };
  const { get } = fakeRegistry({ manifests });
  const fork = { ...failedRun({ id: 1, head_repository: { full_name: "someone/purlis" } }), digests: undefined };
  const runs = [...releaseRuns.filter((run) => run.id !== cancelled.id), { ...cancelled }, fork];
  const downloaded = [];
  const api = (path) => {
    if (path.startsWith(`repos/${REPO}/actions/workflows/${WORKFLOW}/runs?created=>=2026-10-31`)) return { total_count: runs.length, workflow_runs: runs };
    if (path.startsWith(`repos/${REPO}/actions/runs/${cancelled.id}/artifacts`)) {
      return { total_count: 1, artifacts: [{ ...recordedArtifacts.find((a) => a.name === "digest-amd64"), workflow_run: { id: cancelled.id } }] };
    }
    if (path.startsWith("orgs/purlis/packages/container/purlis-runner/versions")) return versionsOf(recordedRegistry, [untagged(LEFTOVER, "2026-11-03T04:40:00Z")]);
    throw new Error(`unexpected ${path}`);
  };
  const download = (run, artifact) => {
    downloaded.push(`${run.id}/${artifact.name}`);
    return `${LEFTOVER}\n`;
  };
  const found = await report({ org: "purlis", name: "purlis-runner", repo: REPO, workflow: WORKFLOW, now: NOW, api, get, download });
  assert.deepEqual(found.removable, [{ id: 700_000_001, digest: LEFTOVER, run: cancelled.id }]);
  assert.deepEqual(found.keep, []);
  // Only the failed run that could publish had its artifacts read: not the fork's.
  assert.deepEqual(downloaded, [`${cancelled.id}/digest-amd64`]);
});
