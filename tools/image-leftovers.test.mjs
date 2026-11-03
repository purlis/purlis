// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Which untagged versions `image-leftovers.yml` (#4930) may delete, read from recorded answers
// (`tools/image-leftovers-fixtures/`, recorded 2026-11-03): the runner package's tags and the
// manifests they reach, read anonymously from the registry; release runs and one run's artifacts
// from the Actions API; and the two digests that run recorded. The packages API's list of
// versions could not be recorded (the recording token had no `read:packages`), so `versionsOf`
// builds it from the registry's graph in the shape that API documents.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import { BOUND_MS, CAP, FAILED, isPlainImage, leftovers, reachable, runDigests } from "./image-leftovers.mjs";

const fixture = (name) =>
  JSON.parse(readFileSync(new URL(`./image-leftovers-fixtures/${name}`, import.meta.url), "utf8"));
const registry = fixture("registry-runner.json");
const releaseRuns = fixture("release-runs.json").workflow_runs;
const recorded = fixture("digests-41581143090.json");

const DAY = 24 * 60 * 60 * 1000;
const NOW = Date.parse("2026-11-08T12:00:00Z");
const LEFTOVER = `sha256:${"e".repeat(64)}`;
const OTHER_LEFTOVER = `sha256:${"f".repeat(64)}`;

/** The packages API's list of versions: one per tagged digest and per manifest a tag reaches. */
function versionsOf({ tags, manifests }, extra = []) {
  const byDigest = new Map();
  for (const [tag, digest] of Object.entries(tags)) {
    byDigest.set(digest, [...(byDigest.get(digest) ?? []), tag]);
  }
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
const plain = { mediaType: "application/vnd.oci.image.manifest.v1+json", config: { mediaType: "application/vnd.oci.image.config.v1+json" } };
const done = releaseRuns.filter((run) => run.status === "completed");

function select(overrides = {}) {
  return leftovers({
    versions: versionsOf(registry, [untagged(LEFTOVER, "2026-11-03T04:40:00Z")]),
    kept: reachable(registry),
    runs: [...done.filter((run) => run.id !== cancelled.id), failedRun()],
    candidates: { [LEFTOVER]: plain },
    now: NOW,
    ...overrides,
  });
}

test("every manifest a tag reaches is kept: the indexes' own images, signatures and attestations", () => {
  const kept = reachable(registry);
  // The architecture images of the recorded release, which no tag names but its index holds.
  assert.ok(kept.has(recorded.amd64) && kept.has(recorded.arm64));
  // Its attestation and signature manifests, which only the `sha256-<digest>` index holds.
  assert.ok(kept.has("sha256:1b3fe7493ce02fd2eed8269310c9df18e53b01b621181fed025fa115ec1b6fa9"));
  assert.ok(kept.has("sha256:272d2d4186eaeef0d28177512d8599cd23c5a98a45924ecf5dfa4c89e7acd0a7"));
  // `main`'s own images.
  assert.ok(kept.has("sha256:d393ae758105fcb3d4f0df64935e4d0a59872470c996766f3135f338eb3b2fea"));
  assert.ok(!kept.has(LEFTOVER));
});

test("a digest a referrers tag names is kept even when no index holds it", () => {
  const tags = { [`sha256-${"e".repeat(64)}`]: "sha256:4a0bd4176b31f166f30346eb7ad8cfeb4fe4610540e3e468a1d36ed47d5a661f" };
  assert.ok(reachable({ tags, manifests: registry.manifests }).has(LEFTOVER));
  const signature = { [`sha256-${"e".repeat(64)}.sig`]: "sha256:4a0bd4176b31f166f30346eb7ad8cfeb4fe4610540e3e468a1d36ed47d5a661f" };
  assert.ok(reachable({ tags: signature, manifests: registry.manifests }).has(LEFTOVER));
});

test("the graph cannot be told when a tagged manifest or an index it holds was not read", () => {
  assert.throws(
    () => reachable({ tags: { main: LEFTOVER }, manifests: {} }),
    /cannot tell what main reaches: its manifest .* was not read/,
  );
  const holdsAnIndex = {
    [OTHER_LEFTOVER]: { mediaType: "application/vnd.oci.image.index.v1+json", manifests: [{ digest: LEFTOVER, mediaType: "application/vnd.oci.image.index.v1+json" }] },
  };
  assert.throws(() => reachable({ tags: { main: OTHER_LEFTOVER }, manifests: holdsAnIndex }), /was not read/);
});

test("an untagged image a failed run recorded, that nothing reaches, is removed three days after the run", () => {
  const { remove, keep } = select();
  assert.deepEqual(remove, [{ id: 700_000_001, digest: LEFTOVER, run: cancelled.id }]);
  assert.deepEqual(keep, []);
});

test("the recorded release's own images are never candidates: a tag reaches them", () => {
  const run = { ...cancelled, digests: [recorded.amd64, recorded.arm64] };
  const versions = versionsOf(registry).map((v) =>
    v.name === recorded.amd64 ? { ...v, created_at: "2026-11-03T04:40:00Z" } : v,
  );
  const { remove } = select({ versions, runs: [run], candidates: {} });
  assert.deepEqual(remove, []);
});

test("an untagged version no failed run recorded is kept", () => {
  const { remove, keep } = select({ runs: done.filter((run) => run.id !== cancelled.id) });
  assert.deepEqual(remove, []);
  assert.deepEqual(keep, [{ digest: LEFTOVER, why: "no failed publishing run recorded it" }]);
});

test("a version a run that succeeded also recorded is kept, whatever else recorded it", () => {
  const succeeded = { ...done.find((run) => run.conclusion === "success"), digests: [LEFTOVER] };
  const { remove, keep } = select({ runs: [failedRun(), succeeded] });
  assert.deepEqual(remove, []);
  assert.match(keep[0].why, /run 41581143090, which did not fail, recorded it/);
});

test("only a run that failed, was cancelled or timed out counts as failed", () => {
  assert.deepEqual([...FAILED].sort(), ["cancelled", "failure", "timed_out"]);
  for (const conclusion of ["success", "skipped", "neutral", "action_required", "stale", null]) {
    const { remove } = select({ runs: [failedRun({ conclusion })] });
    assert.deepEqual(remove, [], String(conclusion));
  }
});

test("nothing is removed while a run of the publishing workflow is under way", () => {
  // The recorded list: one release in progress, one pending.
  const { remove, keep, busy } = select({ runs: [...releaseRuns.filter((run) => run.id !== cancelled.id), failedRun()] });
  assert.deepEqual(remove, []);
  assert.deepEqual(busy, [41586694683, 41583835270]);
  assert.match(keep[0].why, /a publishing run is under way \(41586694683, 41583835270\)/);
});

test("a run that ended less than three days ago keeps what it recorded, so a rerun can still use it", () => {
  assert.equal(BOUND_MS, 3 * DAY);
  const ended = Date.parse(cancelled.updated_at);
  assert.deepEqual(select({ now: ended + 3 * DAY - 1 }).remove, []);
  assert.match(select({ now: ended + 3 * DAY - 1 }).keep[0].why, /ended less than 3 days ago/);
  assert.equal(select({ now: ended + 3 * DAY }).remove.length, 1);
});

test("a version not pushed while its run ran is kept", () => {
  for (const created of ["2026-11-03T04:21:59Z", "2026-11-03T04:58:23Z"]) {
    const { remove, keep } = select({ versions: versionsOf(registry, [untagged(LEFTOVER, created)]) });
    assert.deepEqual(remove, [], created);
    assert.match(keep[0].why, /was not pushed while run 41577697984 ran/);
  }
});

test("only a plain image manifest is removed: an index, a referrer or an artifact is kept", () => {
  const index = { mediaType: "application/vnd.oci.image.index.v1+json", manifests: [] };
  const referrer = registry.manifests["sha256:6680ef18840340309996dae95cafc0662cf30b3f8c20b818691c457a06cd15cc"];
  const artifact = { ...plain, artifactType: "application/vnd.example" };
  const docker = { mediaType: "application/vnd.docker.distribution.manifest.v2+json", config: { mediaType: "application/vnd.docker.container.image.v1+json" } };
  assert.ok(isPlainImage(plain) && isPlainImage(docker));
  assert.ok(isPlainImage(registry.manifests[recorded.amd64]), "the recorded architecture image is one");
  for (const manifest of [index, referrer, artifact, undefined, null, {}]) {
    assert.ok(!isPlainImage(manifest), JSON.stringify(manifest));
    const { remove, keep } = select({ candidates: { [LEFTOVER]: manifest } });
    assert.deepEqual(remove, []);
    assert.match(keep[0].why, /is not a plain image manifest/);
  }
});

test("more candidates than the cap removes nothing and says so", () => {
  assert.equal(CAP, 4);
  const many = Array.from({ length: CAP + 1 }, (_, i) => `sha256:${String(i).repeat(64)}`);
  assert.throws(
    () =>
      select({
        versions: versionsOf(registry, many.map((d, i) => untagged(d, "2026-11-03T04:40:00Z", 700_000_100 + i))),
        runs: [failedRun({ digests: many })],
        candidates: Object.fromEntries(many.map((d) => [d, plain])),
      }),
    /5 versions to remove, more than the 4 a run may: nothing is removed/,
  );
});

test("an answer that cannot be read stops the selection", () => {
  assert.throws(() => select({ versions: "nope" }), /the list of versions is not a list/);
  assert.throws(() => select({ versions: [{ ...untagged(LEFTOVER, "yesterday") }] }), /created_at/);
  assert.throws(() => select({ versions: [{ ...untagged("latest", "2026-11-03T04:40:00Z") }] }), /not a sha256 digest/);
  assert.throws(() => select({ runs: [failedRun({ updated_at: "soon" })] }), /updated_at/);
  assert.throws(() => select({ runs: [failedRun({ digests: ["sha256:abc"] })] }), /not a sha256 digest/);
  assert.throws(() => select({ now: Number.NaN }), /now/);
});

test("a run's digests are read from its digest-<arch> artifacts, by name, and nothing else", () => {
  const listing = fixture("artifacts-41581143090.json").artifacts;
  const read = [];
  const digests = runDigests({
    artifacts: listing,
    download: (artifact) => {
      read.push(artifact.name);
      return `${recorded[artifact.name.slice("digest-".length)]}\n`;
    },
  });
  assert.deepEqual(read.sort(), ["digest-amd64", "digest-arm64"]);
  assert.deepEqual(digests.sort(), [recorded.amd64, recorded.arm64].sort());
  assert.throws(
    () => runDigests({ artifacts: [listing.find((a) => a.name === "digest-amd64")], download: () => "sha256:abc" }),
    /digest-amd64 holds no sha256 digest/,
  );
  // An expired artifact cannot be read, and says nothing about what the run pushed.
  const expired = listing.map((a) => ({ ...a, expired: true }));
  assert.deepEqual(runDigests({ artifacts: expired, download: () => assert.fail("read an expired artifact") }), []);
});
