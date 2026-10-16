// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The engine manifest the scheduled workflow signs (#2073) is the shape the browser engine
// trusts (`crates/browser-engine/engine-pin.json`), made from Chrome for Testing's
// last-known-good versions. Run with `node --test tools/`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { PLATFORMS, downloadsOf, manifestOf } from "./engine-manifest.mjs";

const PIN = readFileSync(
  new URL("../crates/browser-engine/engine-pin.json", import.meta.url),
  "utf8",
);

/** Chrome for Testing's answer, as its JSON is shaped, for `version`. */
function lastKnownGood(version, platforms = [...PLATFORMS, "win32", "win64"]) {
  const download = (platform) => ({
    platform,
    url: `https://storage.googleapis.com/chrome-for-testing-public/${version}/${platform}/chrome-${platform}.zip`,
  });
  return {
    timestamp: "2026-10-16T10:20:11.839Z",
    channels: {
      Stable: {
        channel: "Stable",
        version,
        revision: "1697595",
        downloads: { chrome: platforms.map(download), chromedriver: [] },
      },
      Beta: {
        channel: "Beta",
        version: "156.0.8100.10",
        revision: "1",
        downloads: { chrome: [...PLATFORMS].map(download) },
      },
    },
  };
}

test("the manifest made from the pin's own values is the pin, byte for byte", () => {
  const pin = JSON.parse(PIN);
  const digests = Object.fromEntries(pin.downloads.map((d) => [d.platform, d.sha256]));
  const text = manifestOf(
    downloadsOf(lastKnownGood(pin.version)),
    digests,
    new Date(pin.issued),
  );
  assert.equal(text, PIN);
});

test("only the stable channel's archives for the four platforms purlis runs on are taken", () => {
  const taken = downloadsOf(lastKnownGood("155.0.8059.84"));
  assert.equal(taken.version, "155.0.8059.84");
  assert.deepEqual(
    taken.downloads.map((d) => d.platform),
    ["mac-arm64", "mac-x64", "linux64", "linux-arm64"],
  );
});

test("an answer missing a platform purlis runs on is refused", () => {
  assert.throws(
    () => downloadsOf(lastKnownGood("155.0.8059.84", ["mac-arm64", "mac-x64", "linux64"])),
    /linux-arm64/,
  );
});

test("an archive anywhere but Chrome for Testing's own bucket, for that version, is refused", () => {
  for (const url of [
    "https://example.com/chrome-for-testing-public/155.0.8059.84/linux64/chrome-linux64.zip",
    "http://storage.googleapis.com/chrome-for-testing-public/155.0.8059.84/linux64/chrome-linux64.zip",
    "https://storage.googleapis.com/chrome-for-testing-public/155.0.8059.83/linux64/chrome-linux64.zip",
    "https://storage.googleapis.com/chrome-for-testing-public/155.0.8059.84/linux64/other.zip",
  ]) {
    const answer = lastKnownGood("155.0.8059.84");
    answer.channels.Stable.downloads.chrome.find((d) => d.platform === "linux64").url = url;
    assert.throws(() => downloadsOf(answer), /linux64/, url);
  }
});

test("a version that is not four numbers is refused", () => {
  for (const version of ["", "155", "155.0.8059", "155.0.8059.x", "155.0.8059.84 ", "../1.2.3.4"]) {
    assert.throws(() => downloadsOf(lastKnownGood(version)), /version/, JSON.stringify(version));
  }
});

test("a digest that is not a SHA-256 is refused", () => {
  const taken = downloadsOf(lastKnownGood("155.0.8059.84"));
  const digests = Object.fromEntries(PLATFORMS.map((p) => [p, "a".repeat(64)]));
  assert.doesNotThrow(() => manifestOf(taken, digests, new Date()));
  assert.throws(() => manifestOf(taken, { ...digests, linux64: "A".repeat(64) }, new Date()), /linux64/);
  const { "mac-x64": _, ...missing } = digests;
  assert.throws(() => manifestOf(taken, missing, new Date()), /mac-x64/);
});

test("the issue time is written to the second, in UTC", () => {
  const taken = downloadsOf(lastKnownGood("155.0.8059.84"));
  const digests = Object.fromEntries(PLATFORMS.map((p) => [p, "b".repeat(64)]));
  const text = manifestOf(taken, digests, new Date("2026-10-16T05:17:42.918Z"));
  assert.equal(JSON.parse(text).issued, "2026-10-16T05:17:42Z");
});
