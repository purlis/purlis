#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The untagged images a failed publishing run left in an image package (#4930), and their removal.
//
// `runner-image.yml` and `operator-image.yml` push each architecture's checked image by digest,
// untagged, and `index` names them (#4512). When a later step of the same run fails (the other
// architecture, `index`, `sbom`, `attest`, `outside`), an image it pushed can stay behind:
// checked, but untagged, unsigned and named by nothing. Untagged is not enough to tell one: an
// index's own architecture images are untagged, and so are its signatures and attestations, which
// the registry keeps under a `sha256-<digest>` referrers tag. So a version is removed only when
// **all** of these hold:
//
//   1. no run of the package's publishing workflow is queued or under way;
//   2. it has no tag, and nothing any tag reaches holds it: no index's child, at any depth, and
//      no digest a `sha256-<digest>` referrers tag names;
//   3. a run of the publishing workflow that failed, was cancelled or timed out recorded it as
//      its own push (its `digest-<arch>` artifact), and no run that did not fail recorded it;
//   4. it was pushed while that run ran, and that run ended at least BOUND_MS ago;
//   5. its manifest is a plain image manifest: no index, no referrer, no artifact;
//   6. no more than CAP versions qualify; past that, nothing is removed.
//
// Any answer that cannot be read stops the run with nothing removed.
//
//   node tools/image-leftovers.mjs plan WORKFLOW
//       Says what would be removed, and why each other untagged version stays. Removes nothing.
//   node tools/image-leftovers.mjs remove WORKFLOW
//       The same, then removes each version, asking 1 and 2 again just before each one.
//
// WORKFLOW is the file of the workflow whose runs push the package: `release.yml` for the runner
// image (it calls `runner-image.yml`), `operator-image.yml` for the operator's. Needs IMAGE,
// GH_TOKEN, GITHUB_REPOSITORY and GITHUB_REPOSITORY_OWNER, as `image-leftovers.yml` sets them.
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { packageOf } from "./image-registry.mjs";

/** How long after its run ended a leftover is kept, so a rerun of the failed jobs can still use it. */
export const BOUND_MS = 3 * 24 * 60 * 60 * 1000;
/** The most versions one run removes. A failed run leaves at most two. */
export const CAP = 4;
/** The conclusions of a run that failed. */
export const FAILED = new Set(["failure", "cancelled", "timed_out"]);

const DIGEST = /^sha256:[0-9a-f]{64}$/;
const REFERRERS_TAG = /^sha256-([0-9a-f]{64})(?:\.[a-z]+)?$/;
const INDEX_TYPES = new Set([
  "application/vnd.oci.image.index.v1+json",
  "application/vnd.docker.distribution.manifest.list.v2+json",
]);
const IMAGE_TYPES = new Set([
  "application/vnd.oci.image.manifest.v1+json",
  "application/vnd.docker.distribution.manifest.v2+json",
]);
const CONFIG_TYPES = new Set([
  "application/vnd.oci.image.config.v1+json",
  "application/vnd.docker.container.image.v1+json",
]);

function digestOf(value, what) {
  if (typeof value !== "string" || !DIGEST.test(value)) {
    throw new Error(`${what} is ${JSON.stringify(value ?? null)}, not a sha256 digest`);
  }
  return value;
}

function timeOf(value, what) {
  const at = typeof value === "string" ? Date.parse(value) : Number.NaN;
  if (Number.isNaN(at)) throw new Error(`${what} is ${JSON.stringify(value ?? null)}, not a time`);
  return at;
}

const isIndex = (manifest) => INDEX_TYPES.has(manifest?.mediaType);

/**
 * Every digest a tag reaches: each tag's own manifest, every child of every index under it, and
 * each digest a `sha256-<digest>` referrers tag names. `tags` maps a tag to its digest; `manifests`
 * maps a digest to its manifest, and holds every tagged one and every index a tag reaches. Throws
 * when one of those is missing: what it holds cannot be told.
 */
export function reachable({ tags, manifests }) {
  const kept = new Set();
  const visit = (digest, from) => {
    if (kept.has(digest)) return;
    kept.add(digest);
    const manifest = manifests[digest];
    if (manifest === undefined) throw new Error(`cannot tell what ${from} reaches: its manifest ${digest} was not read`);
    if (!isIndex(manifest)) return;
    if (!Array.isArray(manifest.manifests)) throw new Error(`cannot tell what ${from} reaches: index ${digest} lists no manifests`);
    for (const child of manifest.manifests) {
      const childDigest = digestOf(child?.digest, `a child of ${digest}`);
      if (INDEX_TYPES.has(child.mediaType)) visit(childDigest, from);
      else kept.add(childDigest);
    }
  };
  for (const [tag, digest] of Object.entries(tags)) {
    visit(digestOf(digest, `the tag ${tag}`), tag);
    const subject = REFERRERS_TAG.exec(tag)?.[1];
    if (subject) kept.add(`sha256:${subject}`);
  }
  return kept;
}

/** A single image's manifest: not an index, not a referrer, not an artifact. */
export function isPlainImage(manifest) {
  return (
    IMAGE_TYPES.has(manifest?.mediaType) &&
    manifest.subject == null &&
    manifest.artifactType == null &&
    CONFIG_TYPES.has(manifest.config?.mediaType)
  );
}

/**
 * The versions to remove, and why every other untagged one stays.
 *
 * `versions` is the packages API's list; `kept` is `reachable(...)`; `runs` are the publishing
 * workflow's recent runs as the Actions API lists them, each with the `digests` its artifacts
 * recorded (empty for a run that recorded none); `candidates` maps a digest to its manifest, for
 * the untagged versions not kept. Returns `{ remove: [{ id, digest, run }], keep: [{ digest, why }],
 * busy: [run ids] }`. Throws when an answer cannot be read, and when more than CAP qualify.
 */
export function leftovers({ versions, kept, runs, candidates, now, bound = BOUND_MS, cap = CAP }) {
  if (!Number.isFinite(now)) throw new Error(`now is ${now}, not a time`);
  if (!Array.isArray(versions)) throw new Error("the list of versions is not a list");
  if (!Array.isArray(runs)) throw new Error("the list of runs is not a list");

  const busy = runs.filter((run) => run.status !== "completed").map((run) => run.id);
  const recordedBy = new Map();
  for (const run of runs) {
    const ran = { started: timeOf(run.run_started_at, `run ${run.id}'s run_started_at`), ended: timeOf(run.updated_at, `run ${run.id}'s updated_at`) };
    for (const digest of run.digests ?? []) {
      digestOf(digest, `a digest run ${run.id} recorded`);
      recordedBy.set(digest, [...(recordedBy.get(digest) ?? []), { ...run, ...ran }]);
    }
  }

  const remove = [];
  const keep = [];
  for (const version of versions) {
    const digest = digestOf(version?.name, `version ${version?.id}'s name`);
    const created = timeOf(version.created_at, `version ${version.id}'s created_at`);
    const tags = version.metadata?.container?.tags;
    if (!Array.isArray(tags)) throw new Error(`version ${version.id} lists no tags`);
    if (tags.length > 0 || kept.has(digest)) continue;

    const stay = (why) => keep.push({ digest, why });
    if (busy.length > 0) {
      stay(`a publishing run is under way (${busy.join(", ")})`);
      continue;
    }
    const recorders = recordedBy.get(digest) ?? [];
    const notFailed = recorders.find((run) => run.status !== "completed" || !FAILED.has(run.conclusion));
    if (notFailed) {
      stay(`run ${notFailed.id}, which did not fail, recorded it`);
      continue;
    }
    if (recorders.length === 0) {
      stay("no failed publishing run recorded it");
      continue;
    }
    const run = recorders.find((r) => r.started <= created && created <= r.ended);
    if (!run) {
      stay(`it was not pushed while run ${recorders.map((r) => r.id).join(", ")} ran`);
      continue;
    }
    if (now - run.ended < bound || now - created < bound) {
      stay(`run ${run.id} ended less than ${bound / (24 * 60 * 60 * 1000)} days ago`);
      continue;
    }
    if (!isPlainImage(candidates[digest])) {
      stay("it is not a plain image manifest");
      continue;
    }
    remove.push({ id: version.id, digest, run: run.id });
  }

  if (remove.length > cap) {
    throw new Error(
      `${remove.length} versions to remove, more than the ${cap} a run may: nothing is removed. Read them: ${remove.map((r) => r.digest).join(" ")}`,
    );
  }
  return { remove, keep, busy };
}

/**
 * The digests a run recorded as its own pushes: the one line of each unexpired `digest-amd64` and
 * `digest-arm64` artifact, by name. `download(artifact)` answers the artifact's text.
 */
export function runDigests({ artifacts, download }) {
  const digests = [];
  for (const artifact of artifacts) {
    if (!/^digest-(amd64|arm64)$/.test(artifact.name) || artifact.expired) continue;
    const text = String(download(artifact)).trim();
    if (!DIGEST.test(text)) throw new Error(`${artifact.name} holds no sha256 digest`);
    digests.push(text);
  }
  return digests;
}

// ---- What the forge and the registry answer ----

function ghApi(args) {
  try {
    return execFileSync("gh", ["api", ...args], { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], maxBuffer: 64 * 1024 * 1024 });
  } catch (error) {
    throw new Error(`gh api ${args.at(-1)}: ${String(error.stderr || error.message).trim()}`);
  }
}

/** Every page of a list the forge answers in pages of 100. */
function pages(path, pick) {
  const all = [];
  for (let page = 1; ; page += 1) {
    const got = pick(JSON.parse(ghApi([`${path}${path.includes("?") ? "&" : "?"}per_page=100&page=${page}`])));
    if (!Array.isArray(got)) throw new Error(`${path} did not answer a list`);
    all.push(...got);
    if (got.length < 100) return all;
  }
}

const ACCEPT = [...INDEX_TYPES, ...IMAGE_TYPES].join(", ");

async function registry(repository) {
  const answer = await fetch(`https://ghcr.io/token?scope=repository:${repository}:pull`, { signal: AbortSignal.timeout(30_000) });
  if (answer.status !== 200) throw new Error(`the registry's token endpoint answered ${answer.status}`);
  const { token } = await answer.json();
  if (typeof token !== "string" || token === "") throw new Error("the registry gave no token");
  const get = async (url, accept) => {
    const response = await fetch(url, { headers: { Authorization: `Bearer ${token}`, Accept: accept }, signal: AbortSignal.timeout(30_000) });
    if (response.status !== 200) throw new Error(`${url} answered ${response.status}`);
    return response;
  };
  return {
    async tags() {
      const tags = [];
      for (let url = `https://ghcr.io/v2/${repository}/tags/list?n=1000`; url; ) {
        const response = await get(url, "application/json");
        const { tags: page } = await response.json();
        if (!Array.isArray(page)) throw new Error(`${url} listed no tags`);
        tags.push(...page);
        const next = /<([^>]+)>;\s*rel="next"/.exec(response.headers.get("link") ?? "")?.[1];
        url = next ? new URL(next, "https://ghcr.io").href : null;
      }
      return tags;
    },
    async manifest(reference) {
      const response = await get(`https://ghcr.io/v2/${repository}/manifests/${reference}`, ACCEPT);
      const digest = digestOf(response.headers.get("docker-content-digest"), `the digest the registry gave ${reference}`);
      if (reference.startsWith("sha256:") && digest !== reference) throw new Error(`${reference} was answered with ${digest}`);
      return { digest, manifest: JSON.parse(await response.text()) };
    },
  };
}

/** The registry's tags and every manifest they reach, read as a puller who has not signed in. */
async function graph(reg) {
  const tags = {};
  const manifests = {};
  const read = async (reference) => {
    const { digest, manifest } = await reg.manifest(reference);
    manifests[digest] = manifest;
    if (isIndex(manifest)) {
      for (const child of manifest.manifests ?? []) {
        if (INDEX_TYPES.has(child?.mediaType) && !(child.digest in manifests)) await read(digestOf(child.digest, "a child"));
      }
    }
    return digest;
  };
  for (const tag of await reg.tags()) tags[tag] = await read(tag);
  return { tags, manifests };
}

function publishingRuns({ repo, workflow, since }) {
  const runs = pages(`repos/${repo}/actions/workflows/${workflow}/runs?created=>=${since}`, (a) => a.workflow_runs);
  return runs.map((run) => {
    if (run.status !== "completed" || !FAILED.has(run.conclusion)) return { ...run, digests: [] };
    const artifacts = pages(`repos/${repo}/actions/runs/${run.id}/artifacts`, (a) => a.artifacts);
    const download = (artifact) => {
      const dir = mkdtempSync(join(tmpdir(), "leftovers-"));
      try {
        execFileSync("gh", ["run", "download", String(run.id), "-R", repo, "-n", artifact.name, "-D", dir], { stdio: ["ignore", "pipe", "pipe"] });
        const files = readdirSync(dir);
        if (files.length !== 1) throw new Error(`${artifact.name} of run ${run.id} holds ${files.length} files`);
        return readFileSync(join(dir, files[0]), "utf8");
      } finally {
        rmSync(dir, { recursive: true, force: true });
      }
    };
    return { ...run, digests: runDigests({ artifacts, download }) };
  });
}

async function select({ org, name, repo, workflow, now }) {
  // Runs that ended in the last 8 days: a run's `digest-<arch>` artifacts are kept 7.
  const since = new Date(now - 8 * 24 * 60 * 60 * 1000).toISOString().slice(0, 10);
  const runs = publishingRuns({ repo, workflow, since });
  const versions = pages(`orgs/${org}/packages/container/${name}/versions`, (a) => a);
  const reg = await registry(`${org}/${name}`);
  const kept = reachable(await graph(reg));
  const candidates = {};
  for (const version of versions) {
    const tags = version?.metadata?.container?.tags ?? [];
    if (tags.length === 0 && !kept.has(version.name)) {
      candidates[version.name] = (await reg.manifest(digestOf(version.name, `version ${version.id}'s name`))).manifest;
    }
  }
  return { ...leftovers({ versions, kept, runs, candidates, now }), reg };
}

async function main([command, workflow, ...rest]) {
  const org = process.env.GITHUB_REPOSITORY_OWNER;
  const repo = process.env.GITHUB_REPOSITORY;
  if (!org || !repo) throw new Error("GITHUB_REPOSITORY and GITHUB_REPOSITORY_OWNER have to be set");
  if (!["plan", "remove"].includes(command) || !/^[a-z0-9-]+\.yml$/.test(workflow ?? "") || rest.length > 0) {
    throw new Error("usage: image-leftovers.mjs plan|remove WORKFLOW");
  }
  const pkg = packageOf({ image: process.env.IMAGE, owner: org });
  const now = Date.now();
  const { remove, keep, busy, reg } = await select({ ...pkg, repo, workflow, now });
  for (const { digest, why } of keep) console.log(`keep ${digest}: ${why}`);
  for (const { digest, run } of remove) console.log(`${command === "remove" ? "remove" : "would remove"} ${digest}: left by run ${run}`);
  if (remove.length === 0) console.log(busy.length > 0 ? `nothing removed: runs ${busy.join(", ")} are under way` : "nothing to remove");
  if (command !== "remove") return;

  for (const { id, digest } of remove) {
    // Asked again just before each removal: a run may have started, or a tag moved, since.
    const since = new Date(Date.now() - 8 * 24 * 60 * 60 * 1000).toISOString().slice(0, 10);
    const started = pages(`repos/${repo}/actions/workflows/${workflow}/runs?created=>=${since}`, (a) => a.workflow_runs)
      .filter((run) => run.status !== "completed");
    if (started.length > 0) throw new Error(`run ${started[0].id} is under way: ${digest} and the rest are kept`);
    if (reachable(await graph(reg)).has(digest)) throw new Error(`a tag now reaches ${digest}: it and the rest are kept`);
    try {
      ghApi(["-X", "DELETE", `orgs/${pkg.org}/packages/container/${pkg.name}/versions/${id}`]);
    } catch (error) {
      // The job's token deletes only when the repository holds the Admin role on the package
      // (its settings, Manage Actions access): a setting of the organisation's, made by hand.
      const why = /HTTP 403/.test(error.message)
        ? " The repository needs the Admin role in the package's Manage Actions access."
        : "";
      throw new Error(`${error.message}${why}`);
    }
    console.log(`removed ${digest} (version ${id})`);
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    await main(process.argv.slice(2));
  } catch (error) {
    console.error(`::error::${error.message}`);
    process.exit(1);
  }
}
