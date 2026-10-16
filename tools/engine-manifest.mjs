#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The browser engine's manifest, as `.github/workflows/engine-manifest.yml` signs it each day
// (#2073): Chrome for Testing's last-known-good stable version, and for each platform purlis
// runs on, its archive's address and SHA-256, in the shape the engine trusts
// (`crates/browser-engine/engine-pin.json`, `purlis_browser_engine::install::Manifest`).
//
//   node tools/engine-manifest.mjs <out.json>
//
// Downloads each archive (a few hundred megabytes in all) and hashes it as it arrives. Exits 1
// with nothing written when any answer is not what it should be, so nothing is signed on a
// guess.
//
// **Issued** is when this ran: the manifest says "as of now, this version is current", which is
// what the engine's age is counted from. Signed each day, the same version issued later keeps
// an engine that is still current from being refused for its age.
import { createHash } from "node:crypto";
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

/** Where Chrome for Testing says which versions are current, with their archives. */
export const SOURCE =
  "https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions-with-downloads.json";

/** The platforms purlis runs the engine on, in the pin's order (`install::Platform`). */
export const PLATFORMS = ["mac-arm64", "mac-x64", "linux64", "linux-arm64"];

/** Chrome for Testing's own bucket: no archive is taken from anywhere else. */
const BUCKET = "https://storage.googleapis.com/chrome-for-testing-public/";

/** The largest archive downloaded, as `install::MAX_ARCHIVE_BYTES`. */
const MAX_ARCHIVE_BYTES = 2 ** 30;

/**
 * The stable version and each platform's archive address, from Chrome for Testing's
 * last-known-good answer. Throws, naming what is wrong, when the version is not four numbers or
 * a platform's archive is missing or anywhere but `BUCKET/<version>/<platform>/`.
 */
export function downloadsOf(lastKnownGood) {
  const stable = lastKnownGood?.channels?.Stable;
  const version = stable?.version;
  if (typeof version !== "string" || !/^\d+\.\d+\.\d+\.\d+$/.test(version)) {
    throw new Error(`the stable version ${JSON.stringify(version)} is not a version`);
  }
  const offered = stable.downloads?.chrome ?? [];
  const downloads = PLATFORMS.map((platform) => {
    const url = offered.find((d) => d?.platform === platform)?.url;
    const expected = `${BUCKET}${version}/${platform}/chrome-${platform}.zip`;
    if (url !== expected) {
      throw new Error(`the ${platform} archive is ${JSON.stringify(url)}, not ${expected}`);
    }
    return { platform, url };
  });
  return { version, downloads };
}

/**
 * The manifest's text: `taken` (from `downloadsOf`) with each platform's SHA-256 from
 * `digests`, issued at `issued` to the second, laid out as `engine-pin.json` is.
 */
export function manifestOf(taken, digests, issued) {
  const manifest = {
    version: taken.version,
    issued: issued.toISOString().replace(/\.\d+Z$/, "Z"),
    downloads: taken.downloads.map(({ platform, url }) => {
      const sha256 = digests[platform];
      if (typeof sha256 !== "string" || !/^[0-9a-f]{64}$/.test(sha256)) {
        throw new Error(`the ${platform} archive has no SHA-256`);
      }
      return { platform, url, sha256 };
    }),
  };
  return `${JSON.stringify(manifest, null, 2)}\n`;
}

/** The SHA-256 of what `url` serves, read as it arrives. */
async function sha256Of(url) {
  const response = await fetch(url, { redirect: "error" });
  if (!response.ok || !response.body) {
    throw new Error(`${url} answered ${response.status}`);
  }
  const hash = createHash("sha256");
  let total = 0;
  for await (const chunk of response.body) {
    total += chunk.length;
    if (total > MAX_ARCHIVE_BYTES) {
      throw new Error(`${url} is larger than any engine`);
    }
    hash.update(chunk);
  }
  return hash.digest("hex");
}

async function main(out) {
  if (!out) {
    throw new Error("usage: node tools/engine-manifest.mjs <out.json>");
  }
  const answer = await fetch(SOURCE, { redirect: "error" });
  if (!answer.ok) {
    throw new Error(`${SOURCE} answered ${answer.status}`);
  }
  const taken = downloadsOf(await answer.json());
  const digests = {};
  for (const { platform, url } of taken.downloads) {
    digests[platform] = await sha256Of(url);
    console.log(`${platform}: ${digests[platform]}`);
  }
  const text = manifestOf(taken, digests, new Date());
  writeFileSync(out, text);
  console.log(`Chrome for Testing ${taken.version}, written to ${out}`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main(process.argv[2]).catch((error) => {
    console.error(`::error::${error.message}`);
    process.exit(1);
  });
}
