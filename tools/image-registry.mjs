#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// What `runner-image.yml` (#2043) and `operator-image.yml` (#2238) ask before they push, tag or
// sign an image into a public package of the organisation (#4498).
//
//   node tools/image-registry.mjs visibility
//       `public`, from the forge's packages API; exits 1 on any other answer, or none. A package
//       that is not there is one of the other answers: a new package starts private, and a
//       release that names an image nobody can pull helps no one.
//   node tools/image-registry.mjs anonymous DIGEST...
//       `served` when the registry serves each digest to a puller who has not signed in; exits 1
//       when it refuses one, or when its answers do not show that it serves.
//   node tools/image-registry.mjs order SHA
//       `main: <why>` when the image of SHA may be tagged `main`, `sha-only: <why>` when it may
//       not (an image of a later commit holds `main`); exits 1 when it cannot place SHA.
//
// Needs IMAGE (`ghcr.io/<owner>/<package>`, the workflow's own name for the image it pushes, so
// the package asked about is the one pushed to), GH_TOKEN (the job's token, which may read the
// package), GITHUB_REPOSITORY and GITHUB_REPOSITORY_OWNER, as the workflows' jobs have them.
// **Every question fails closed:** a guess never pushes, tags or signs.
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

/**
 * The package `image` names: `{ org, name }` for `ghcr.io/<owner>/<name>`, where `<owner>` is the
 * repository's owner. Throws for any other registry, owner or shape, and for a tag or a digest.
 */
export function packageOf({ image, owner }) {
  const org = String(owner ?? "").toLowerCase();
  const match = /^ghcr\.io\/([a-z0-9-]+)\/([a-z0-9][a-z0-9._-]*)$/.exec(String(image ?? ""));
  if (!org || !match || match[1] !== org) {
    throw new Error(
      `IMAGE is ${JSON.stringify(image ?? null)}; it has to be ghcr.io/${org || "<owner>"}/<package>, a package of the repository's owner, with no tag or digest`,
    );
  }
  return { org, name: match[2] };
}

/**
 * The package's visibility: `public`. Throws for any other visibility, for a package that is not
 * there, and when the forge's answer does not say.
 */
export function visibility({ org, name, api }) {
  let text;
  try {
    text = api([`orgs/${org}/packages/container/${name}`]);
  } catch (error) {
    throw new Error(`cannot tell whether ${org}/${name} is public: ${error.message}`);
  }
  let shown;
  try {
    shown = JSON.parse(text).visibility;
  } catch {
    throw new Error(`cannot tell whether ${org}/${name} is public: the forge's answer is not JSON`);
  }
  if (shown !== "public") {
    throw new Error(`the package ${org}/${name} is not public (its visibility is ${JSON.stringify(shown ?? null)})`);
  }
  return "public";
}

/**
 * `served` when the registry serves every one of `digests` of `repository` to a puller who has
 * not signed in. `get(url, headers)` answers `{ status, body }` or throws. Throws when the
 * registry refuses one, and when its answers do not show that it serves.
 */
export async function anonymousPull({ repository, digests, get }) {
  if (digests.length === 0) throw new Error("no digest to ask about");
  const refused = (what) => new Error(`${repository} cannot be pulled without signing in: ${what}`);
  const unclear = (why) => new Error(`cannot show that ${repository} serves an anonymous pull: ${why}`);

  let answer;
  try {
    answer = await get(`https://ghcr.io/token?scope=repository:${repository}:pull`, {});
  } catch (error) {
    throw unclear(`asking for an anonymous token failed: ${error.message}`);
  }
  if (answer.status === 401 || answer.status === 403) throw refused(`the token endpoint answered ${answer.status}`);
  if (answer.status !== 200) throw unclear(`the token endpoint answered ${answer.status}`);
  let token;
  try {
    token = JSON.parse(answer.body).token;
  } catch {
    throw unclear("the token endpoint's answer is not JSON");
  }
  if (typeof token !== "string" || token === "") throw unclear("the token endpoint gave no token");

  const accept = [
    "application/vnd.oci.image.index.v1+json",
    "application/vnd.oci.image.manifest.v1+json",
    "application/vnd.docker.distribution.manifest.v2+json",
  ].join(", ");
  for (const digest of digests) {
    let pulled;
    try {
      pulled = await get(`https://ghcr.io/v2/${repository}/manifests/${digest}`, {
        Authorization: `Bearer ${token}`,
        Accept: accept,
      });
    } catch (error) {
      throw unclear(`asking for ${digest} failed: ${error.message}`);
    }
    if ([401, 403, 404].includes(pulled.status)) throw refused(`${digest} answered ${pulled.status}`);
    if (pulled.status !== 200) throw unclear(`${digest} answered ${pulled.status}`);
  }
  return "served";
}

/** The commit of the image tagged `main` (from its `sha-<commit>` tag), or `null` when none is. */
function commitTaggedMain({ org, name, api }) {
  for (let page = 1; ; page += 1) {
    const listed = JSON.parse(api([`orgs/${org}/packages/container/${name}/versions?per_page=100&page=${page}`]));
    if (!Array.isArray(listed)) throw new Error("the forge's list of the package's versions is not a list");
    if (listed.length === 0) return null;
    for (const version of listed) {
      const tags = version?.metadata?.container?.tags ?? [];
      if (!tags.includes("main")) continue;
      const sha = tags.map((tag) => /^sha-([0-9a-f]{40})$/.exec(tag)?.[1]).find(Boolean);
      if (!sha) throw new Error(`the image tagged main names no commit (its tags: ${tags.join(", ")})`);
      return sha;
    }
  }
}

/**
 * Whether the image of `sha` may be tagged `main`: `{ main, why }`. `main` only moves forward:
 * to a later commit than the one it holds, to the same commit again (a run finishing), or, when
 * `main` was rewritten, to its current tip. Throws when it cannot place `sha`.
 */
export function mainTag({ org, name, repo, sha, api }) {
  const held = commitTaggedMain({ org, name, api });
  if (held === null) return { main: true, why: "no image is tagged main yet" };
  if (held === sha) return { main: true, why: `the image tagged main is already of ${sha}` };
  const relation = JSON.parse(api([`repos/${repo}/compare/${held}...${sha}`])).status;
  if (relation === "ahead") return { main: true, why: `${sha} is after ${held}, which main holds` };
  if (relation === "behind") {
    return { main: false, why: `${sha} is older than ${held}, which main holds, and main only moves forward` };
  }
  if (relation === "diverged") {
    const tip = JSON.parse(api([`repos/${repo}/commits/main`])).sha;
    return tip === sha
      ? { main: true, why: `${held}, which main holds, is not in main's history, and ${sha} is main's tip` }
      : { main: false, why: `${sha} and ${held} diverged, and ${sha} is not main's tip` };
  }
  throw new Error(`the compare of ${held} with ${sha} answered ${JSON.stringify(relation)}, which says nothing about their order`);
}

function ghApi(args) {
  try {
    return execFileSync("gh", ["api", ...args], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
      maxBuffer: 16 * 1024 * 1024,
    });
  } catch (error) {
    throw new Error(`gh api ${args.at(-1)}: ${String(error.stderr || error.message).trim()}`);
  }
}

async function httpGet(url, headers) {
  const response = await fetch(url, { headers, signal: AbortSignal.timeout(30_000) });
  return { status: response.status, body: await response.text() };
}

async function main([command, ...rest]) {
  const org = process.env.GITHUB_REPOSITORY_OWNER;
  const repo = process.env.GITHUB_REPOSITORY;
  if (!org || !repo) throw new Error("GITHUB_REPOSITORY and GITHUB_REPOSITORY_OWNER have to be set");
  const pkg = packageOf({ image: process.env.IMAGE, owner: org });
  if (command === "visibility" && rest.length === 0) {
    console.log(visibility({ ...pkg, api: ghApi }));
  } else if (command === "anonymous" && rest.length > 0) {
    console.log(await anonymousPull({ repository: `${pkg.org}/${pkg.name}`, digests: rest, get: httpGet }));
  } else if (command === "order" && rest.length === 1) {
    const order = mainTag({ ...pkg, repo, sha: rest[0], api: ghApi });
    console.log(`${order.main ? "main" : "sha-only"}: ${order.why}`);
  } else {
    throw new Error("usage: image-registry.mjs visibility | anonymous DIGEST... | order SHA");
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
