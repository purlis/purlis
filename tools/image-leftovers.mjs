#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The untagged images a failed publishing run left in an image package (#4930), as a report.
//
// `runner-image.yml` and `operator-image.yml` push each architecture's checked image by digest,
// untagged, and `index` names them (#4512). When a later step of the same run fails (the other
// architecture, `index`, `sbom`, `attest`, `outside`), an image it pushed can stay behind:
// checked, but untagged, unsigned and named by nothing. Untagged is not enough to tell one: an
// index's own architecture images are untagged, and so are its signatures and attestations, which
// the registry keeps under a `sha256-<digest>` referrers tag. So a version is listed as safe to
// delete only when **all** of these hold:
//
//   1. it has no tag, and nothing a tag reaches holds it: no index's child at any depth, from a
//      tag or from the subject a `sha256-<digest>` referrers tag names, and no such subject;
//   2. a run that could publish (this repository's own run of the publishing workflow, from
//      `main` or a `v*` tag) recorded it as its own push in its `digest-<arch>` artifact, and that
//      run failed, was cancelled or timed out;
//   3. it was pushed while that run ran, and that run ended at least BOUND_MS ago;
//   4. its manifest is a plain image manifest (no index, referrer or artifact), and the
//      registry's referrers API shows nothing referring to it;
//   5. no more than CAP qualify; past that, none is listed.
//
// Any answer that cannot be read stops the report with nothing listed. It deletes nothing: a
// person deletes what it lists, in the package's settings (the person's ruling, D-4930-7).
//
//   node tools/image-leftovers.mjs WORKFLOW
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

const DAY_MS = 24 * 60 * 60 * 1000;
/** How long after its run ended a leftover is left alone, so a rerun of the failed jobs can still use it. */
export const BOUND_MS = 3 * DAY_MS;
/** The most versions one report lists. A failed run leaves at most two. */
export const CAP = 4;
/** The conclusions of a run that failed. */
export const FAILED = new Set(["failure", "cancelled", "timed_out"]);
/** Why an untagged version nothing reaches stays when no failed run in the window recorded it. */
const UNRECORDED = "no failed publishing run recorded it";
/** How many registry requests are in flight at once. */
const PARALLEL = 8;

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

/** The digest a `sha256-<digest>` referrers tag names, or null for any other tag. */
const subjectOf = (tag) => {
  const hex = REFERRERS_TAG.exec(tag)?.[1];
  return hex ? `sha256:${hex}` : null;
};

/**
 * Every digest a tag reaches: each tag's own manifest, every child of every index under it, and
 * each subject a `sha256-<digest>` referrers tag names, with everything under that subject too.
 * `tags` maps a tag to its digest; `manifests` maps a digest to its manifest, and holds every
 * tagged one, every index a tag reaches, and every subject the registry still has. Throws when a
 * tagged manifest or an index under one is missing: what it holds cannot be told.
 */
export function reachable({ tags, manifests }) {
  const kept = new Set();
  const visit = (digest, from, required) => {
    if (kept.has(digest)) return;
    kept.add(digest);
    const manifest = manifests[digest];
    if (manifest === undefined) {
      if (required) throw new Error(`cannot tell what ${from} reaches: its manifest ${digest} was not read`);
      return;
    }
    if (!isIndex(manifest)) return;
    if (!Array.isArray(manifest.manifests)) throw new Error(`cannot tell what ${from} reaches: index ${digest} lists no manifests`);
    for (const child of manifest.manifests) {
      const childDigest = digestOf(child?.digest, `a child of ${digest}`);
      if (INDEX_TYPES.has(child.mediaType)) visit(childDigest, from, true);
      else kept.add(childDigest);
    }
  };
  for (const [tag, digest] of Object.entries(tags)) {
    visit(digestOf(digest, `the tag ${tag}`), tag, true);
    // A subject the registry no longer has is kept all the same, and holds nothing to walk.
    const subject = subjectOf(tag);
    if (subject) visit(subject, tag, false);
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
 * Whether `run` could have published the package: this repository's own run of `workflow`, on
 * `main` from `workflow_run`, or (for `release.yml`) from a pushed `v*` tag. A run of a pull
 * request, a fork, another branch or a dispatch runs code that was not reviewed onto `main`, and
 * its artifacts say nothing about what was pushed.
 */
export function couldPublish(run, { repo, workflow }) {
  if (run?.path !== `.github/workflows/${workflow}`) return false;
  if (run.head_repository?.full_name !== repo) return false;
  if (run.event === "workflow_run" && run.head_branch === "main") return true;
  return workflow === "release.yml" && run.event === "push" && /^v[0-9]/.test(run.head_branch ?? "");
}

/**
 * Conditions 1 to 3: the untagged versions nothing reaches that a failed run that could publish
 * left, at least `bound` after it ended. `versions` is the packages API's list; `kept` is
 * `reachable(...)`; `runs` are the publishing workflow's recent runs as the Actions API lists
 * them, each with the `digests` its artifacts recorded (empty for a run whose artifacts were not
 * read). Returns `{ candidates: [{ id, digest, run }], keep: [{ digest, why }] }`, and throws when
 * an answer cannot be read.
 */
export function leftovers({ versions, kept, runs, repo, workflow, now, bound = BOUND_MS }) {
  if (!Number.isFinite(now)) throw new Error(`now is ${now}, not a time`);
  if (!Array.isArray(versions)) throw new Error("the list of versions is not a list");
  if (!Array.isArray(runs)) throw new Error("the list of runs is not a list");

  const recordedBy = new Map();
  for (const run of runs) {
    if (!couldPublish(run, { repo, workflow })) continue;
    // From the run's creation: an earlier attempt's push is the run's own too.
    const ran = { started: timeOf(run.created_at, `run ${run.id}'s created_at`), ended: timeOf(run.updated_at, `run ${run.id}'s updated_at`) };
    for (const digest of run.digests ?? []) {
      digestOf(digest, `a digest run ${run.id} recorded`);
      recordedBy.set(digest, [...(recordedBy.get(digest) ?? []), { ...run, ...ran }]);
    }
  }

  const candidates = [];
  const keep = [];
  for (const version of versions) {
    const digest = digestOf(version?.name, `version ${version?.id}'s name`);
    const created = timeOf(version.created_at, `version ${version.id}'s created_at`);
    const tags = version.metadata?.container?.tags;
    if (!Array.isArray(tags)) throw new Error(`version ${version.id} lists no tags`);
    if (tags.length > 0 || kept.has(digest)) continue;

    const stay = (why) => keep.push({ digest, why });
    const recorders = recordedBy.get(digest) ?? [];
    const notFailed = recorders.find((run) => run.status !== "completed" || !FAILED.has(run.conclusion));
    if (notFailed) {
      stay(`run ${notFailed.id}, which did not fail, recorded it`);
      continue;
    }
    if (recorders.length === 0) {
      stay(UNRECORDED);
      continue;
    }
    const run = recorders.find((r) => r.started <= created && created <= r.ended);
    if (!run) {
      stay(`it was not pushed while run ${recorders.map((r) => r.id).join(", ")} ran`);
      continue;
    }
    if (now - run.ended < bound) {
      stay(`run ${run.id} ended less than ${bound / DAY_MS} days ago`);
      continue;
    }
    candidates.push({ id: version.id, digest, run: run.id });
  }
  return { candidates, keep };
}

/**
 * Conditions 4 and 5, on the candidates `leftovers` gave: `manifests` maps each one's digest to
 * its manifest, and `referrers` to what the registry's referrers API lists for it (`[]` when the
 * registry has no such API: then the referrers tags, walked by `reachable`, are where its
 * referrers are). Returns `{ removable, keep }`; throws past `cap`.
 */
export function removable({ candidates, manifests, referrers, cap = CAP }) {
  const listed = [];
  const keep = [];
  for (const candidate of candidates) {
    const { digest } = candidate;
    const refs = referrers[digest];
    if (!Array.isArray(refs)) throw new Error(`what refers to ${digest} was not read`);
    if (!isPlainImage(manifests[digest])) keep.push({ digest, why: "it is not a plain image manifest" });
    else if (refs.length > 0) keep.push({ digest, why: `${refs.length} manifests refer to it` });
    else listed.push(candidate);
  }
  if (listed.length > cap) {
    throw new Error(
      `${listed.length} versions qualify, more than the ${cap} a failed run could leave: none is listed. Read them: ${listed.map((r) => r.digest).join(" ")}`,
    );
  }
  return { removable: listed, keep };
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

/**
 * Every item of a list the forge answers in pages of 100 under `key` (the list itself when `key`
 * is null). `api(path)` answers the parsed JSON. Throws when the pages hold fewer than the
 * answer's `total_count` says: the Actions API stops at 1,000.
 */
export function pages(api, path, key) {
  const all = [];
  let total = null;
  for (let page = 1; ; page += 1) {
    const answer = api(`${path}${path.includes("?") ? "&" : "?"}per_page=100&page=${page}`);
    const got = key ? answer?.[key] : answer;
    if (!Array.isArray(got)) throw new Error(`${path} did not answer a list`);
    if (key && typeof answer.total_count === "number") total = answer.total_count;
    all.push(...got);
    if (got.length < 100) break;
  }
  if (total !== null && all.length < total) throw new Error(`${path} listed ${all.length} of ${total}`);
  return all;
}

/** `fn` over `items`, at most `limit` at once, in order. */
async function mapLimit(items, limit, fn) {
  const out = new Array(items.length);
  let next = 0;
  const worker = async () => {
    while (next < items.length) {
      const i = next++;
      out[i] = await fn(items[i]);
    }
  };
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, worker));
  return out;
}

const ACCEPT = [...INDEX_TYPES, ...IMAGE_TYPES].join(", ");

/**
 * The registry, read as a puller who has not signed in. `get(url, headers)` answers
 * `{ status, link, digest, body }` or throws.
 */
export async function registry(repository, get) {
  const answer = await get(`https://ghcr.io/token?scope=repository:${repository}:pull`, {});
  if (answer.status !== 200) throw new Error(`the registry's token endpoint answered ${answer.status}`);
  const { token } = JSON.parse(answer.body);
  if (typeof token !== "string" || token === "") throw new Error("the registry gave no token");
  const ask = (url, accept) => get(url, { Authorization: `Bearer ${token}`, Accept: accept });
  const must = (response, url) => {
    if (response.status !== 200) throw new Error(`${url} answered ${response.status}`);
    return response;
  };
  return {
    async tags() {
      const tags = [];
      for (let url = `https://ghcr.io/v2/${repository}/tags/list?n=1000`; url; ) {
        const response = must(await ask(url, "application/json"), url);
        const page = JSON.parse(response.body).tags;
        if (!Array.isArray(page)) throw new Error(`${url} listed no tags`);
        tags.push(...page);
        const next = /<([^>]+)>;\s*rel="next"/.exec(response.link ?? "")?.[1];
        url = next ? new URL(next, "https://ghcr.io").href : null;
      }
      if (new Set(tags).size !== tags.length) throw new Error("the registry listed a tag twice");
      return tags;
    },
    /** `{ digest, manifest }`, or null when `missingIsNull` and the registry has no such manifest. */
    async manifest(reference, missingIsNull = false) {
      const url = `https://ghcr.io/v2/${repository}/manifests/${reference}`;
      const response = await ask(url, ACCEPT);
      if (missingIsNull && response.status === 404) return null;
      must(response, url);
      const digest = digestOf(response.digest, `the digest the registry gave ${reference}`);
      if (reference.startsWith("sha256:") && digest !== reference) throw new Error(`${reference} was answered with ${digest}`);
      return { digest, manifest: JSON.parse(response.body) };
    },
    /** What the referrers API lists for `digest`; `[]` when the registry has no referrers API. */
    async referrers(digest) {
      const url = `https://ghcr.io/v2/${repository}/referrers/${digest}`;
      const response = await ask(url, "application/vnd.oci.image.index.v1+json");
      // ghcr has no referrers API and answers 404 for every digest (2026-11-03); clients then
      // keep referrers under `sha256-<digest>` tags, which `reachable` walks.
      if (response.status === 404) return [];
      const index = JSON.parse(must(response, url).body);
      if (!Array.isArray(index.manifests)) throw new Error(`${url} listed no manifests`);
      return index.manifests;
    },
  };
}

/** The registry's tags and every manifest `reachable` walks. */
export async function graph(reg) {
  const tags = {};
  const manifests = {};
  const names = await reg.tags();
  const read = await mapLimit(names, PARALLEL, (tag) => reg.manifest(tag));
  names.forEach((tag, i) => {
    tags[tag] = read[i].digest;
    manifests[read[i].digest] = read[i].manifest;
  });
  const subjects = [...new Set(names.map(subjectOf).filter((s) => s && !(s in manifests)))];
  const found = await mapLimit(subjects, PARALLEL, (s) => reg.manifest(s, true));
  for (const answer of found) if (answer) manifests[answer.digest] = answer.manifest;
  // Indexes under an index: rare, read one by one.
  const pending = Object.values(manifests).filter(isIndex);
  while (pending.length > 0) {
    for (const child of pending.pop().manifests ?? []) {
      if (!INDEX_TYPES.has(child?.mediaType) || child.digest in manifests) continue;
      const { digest, manifest } = await reg.manifest(digestOf(child.digest, "a child"));
      manifests[digest] = manifest;
      if (isIndex(manifest)) pending.push(manifest);
    }
  }
  return { tags, manifests };
}

/**
 * The publishing workflow's runs created since `since`, each with the digests its artifacts
 * recorded when it is a failed run that could publish (`download(run, artifact)` answers an
 * artifact's text).
 */
export function publishingRuns({ api, download, repo, workflow, since }) {
  const runs = pages(api, `repos/${repo}/actions/workflows/${workflow}/runs?created=>=${since}`, "workflow_runs");
  return runs.map((run) => {
    if (run.status !== "completed" || !FAILED.has(run.conclusion) || !couldPublish(run, { repo, workflow })) {
      return { ...run, digests: [] };
    }
    const artifacts = pages(api, `repos/${repo}/actions/runs/${run.id}/artifacts`, "artifacts");
    return { ...run, digests: runDigests({ artifacts, download: (artifact) => download(run, artifact) }) };
  });
}

/** The report: `{ removable, keep }` for the package `org/name`, pushed by runs of `workflow`. */
export async function report({ org, name, repo, workflow, now, api, get, download }) {
  // Runs created in the last 8 days: a run's `digest-<arch>` artifacts are kept 7.
  const since = new Date(now - 8 * DAY_MS).toISOString().slice(0, 10);
  const runs = publishingRuns({ api, download, repo, workflow, since });
  const versions = pages(api, `orgs/${org}/packages/container/${name}/versions`, null);
  const reg = await registry(`${org}/${name}`, get);
  const kept = reachable(await graph(reg));
  const first = leftovers({ versions, kept, runs, repo, workflow, now });
  const manifests = {};
  const referrers = {};
  for (const { digest } of first.candidates) {
    manifests[digest] = (await reg.manifest(digest)).manifest;
    referrers[digest] = await reg.referrers(digest);
  }
  const second = removable({ candidates: first.candidates, manifests, referrers });
  return { removable: second.removable, keep: [...first.keep, ...second.keep] };
}

function ghApi(path) {
  try {
    return JSON.parse(
      execFileSync("gh", ["api", path], { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], maxBuffer: 64 * 1024 * 1024 }),
    );
  } catch (error) {
    throw new Error(`gh api ${path}: ${String(error.stderr || error.message).trim()}`);
  }
}

function ghDownload(repo) {
  return (run, artifact) => {
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
}

async function httpGet(url, headers) {
  const response = await fetch(url, { headers, signal: AbortSignal.timeout(30_000) });
  return {
    status: response.status,
    link: response.headers.get("link"),
    digest: response.headers.get("docker-content-digest"),
    body: await response.text(),
  };
}

async function main(args) {
  const org = process.env.GITHUB_REPOSITORY_OWNER;
  const repo = process.env.GITHUB_REPOSITORY;
  if (!org || !repo) throw new Error("GITHUB_REPOSITORY and GITHUB_REPOSITORY_OWNER have to be set");
  if (args.length !== 1 || !/^[a-z0-9-]+\.yml$/.test(args[0])) throw new Error("usage: image-leftovers.mjs WORKFLOW");
  const pkg = packageOf({ image: process.env.IMAGE, owner: org });
  const found = await report({ ...pkg, repo, workflow: args[0], now: Date.now(), api: ghApi, get: httpGet, download: ghDownload(repo) });
  // Untagged versions no run in the window recorded are older than this report can tell about
  // (#4512 came before them, or their run's artifacts are gone): counted, not listed one by one.
  const unrecorded = found.keep.filter(({ why }) => why === UNRECORDED);
  for (const { digest, why } of found.keep) if (why !== UNRECORDED) console.log(`keep ${digest}: ${why}`);
  if (unrecorded.length > 0) console.log(`keep ${unrecorded.length} other untagged versions nothing reaches: ${UNRECORDED}`);
  for (const { id, digest, run } of found.removable) {
    console.log(`safe to delete ${digest}: left by run ${run}; https://github.com/orgs/${pkg.org}/packages/container/${pkg.name}/${id}`);
  }
  if (found.removable.length === 0) console.log("nothing to delete");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    await main(process.argv.slice(2));
  } catch (error) {
    console.error(`::error::${error.message}`);
    process.exit(1);
  }
}
