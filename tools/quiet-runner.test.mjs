// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Quieting a macOS runner before the outcome bars measure on it (#4351). Run with
// `node --test tools/`.
import assert from "node:assert/strict";
import { test } from "node:test";

import { busiest, said, settle, topOf } from "./quiet-runner.mjs";

const PS = `
 96.0 /Library/Developer/PrivateFrameworks/CoreSimulator.framework/Resources/bin/simdiskimaged
 61.7 /System/Library/Frameworks/WebKit.framework/XPCServices/com.apple.WebKit.WebContent.xpc/com.apple.WebKit.WebContent
 13.1 mds_stores
  0.0 launchd
`;

test("reads every process from ps, by the program's own name", () => {
  assert.deepEqual(busiest(PS), [
    { cpu: 96, name: "simdiskimaged" },
    { cpu: 61.7, name: "com.apple.WebKit.WebContent" },
    { cpu: 13.1, name: "mds_stores" },
    { cpu: 0, name: "launchd" },
  ]);
});

test("reads a decimal comma, as a locale may print it", () => {
  assert.deepEqual(busiest(" 96,5 simdiskimaged\n"), [{ cpu: 96.5, name: "simdiskimaged" }]);
});

test("says only the busiest few, and judges on them all", async () => {
  const many = Array.from({ length: 8 }, (_, n) => ({
    cpu: 90 - n,
    name: `other-${n}`,
  }));
  assert.equal(topOf(many).length, 6);
  // The service is past the few it would say, and still busy.
  let reads = 0;
  const seen = await settle({
    sample: () => {
      reads += 1;
      return [...many, { cpu: 30, name: "simdiskimaged" }];
    },
    pause: async () => undefined,
    now: () => reads * 5_000,
    services: ["simdiskimaged"],
    belowPercent: 20,
    quietSamples: 1,
    everyMs: 5_000,
    budgetMs: 10_000,
  });
  assert.equal(seen.settled, false);
});

/** Waits over `samples`, one CPU reading of the service each, on a clock that only moves by waiting. */
async function waitOver(samples, options = {}) {
  let at = 0;
  let next = 0;
  const seen = await settle({
    sample: () => {
      const cpu = samples[Math.min(next, samples.length - 1)];
      next += 1;
      return [{ cpu, name: "simdiskimaged" }];
    },
    pause: async (ms) => {
      at += ms;
    },
    now: () => at,
    services: ["simdiskimaged"],
    belowPercent: 20,
    quietSamples: 3,
    everyMs: 5_000,
    budgetMs: 60_000,
    ...options,
  });
  return { ...seen, reads: next };
}

test("is settled once the service has stayed quiet for the samples asked, in a row", async () => {
  const seen = await waitOver([90, 10, 10, 10]);
  assert.equal(seen.settled, true);
  assert.equal(seen.reads, 4);
  assert.equal(seen.waitedMs, 15_000);
});

test("starts counting again when the service gets busy between quiet samples", async () => {
  const seen = await waitOver([10, 10, 90, 10, 10, 10]);
  assert.equal(seen.settled, true);
  assert.equal(seen.reads, 6);
});

test("counts a service that is not running as quiet", async () => {
  const seen = await settle({
    sample: () => [{ cpu: 50, name: "something-else" }],
    pause: async () => undefined,
    now: () => 0,
    services: ["simdiskimaged"],
    belowPercent: 20,
    quietSamples: 2,
    everyMs: 5_000,
    budgetMs: 60_000,
  });
  assert.equal(seen.settled, true);
});

test("stops waiting at its budget, and says it did not settle", async () => {
  const seen = await waitOver([90], { budgetMs: 20_000 });
  assert.equal(seen.settled, false);
  assert.equal(seen.waitedMs, 20_000);
});

test("is not settled on a reading of nothing, which is no reading", async () => {
  let at = 0;
  const seen = await settle({
    sample: () => [],
    pause: async (ms) => {
      at += ms;
    },
    now: () => at,
    services: ["simdiskimaged"],
    belowPercent: 20,
    quietSamples: 1,
    everyMs: 5_000,
    budgetMs: 10_000,
  });
  assert.equal(seen.settled, false);
});

test("waits for every service it is given, Spotlight's beside the disk images'", async () => {
  let at = 0;
  const seen = await settle({
    sample: () => [
      { cpu: 1, name: "simdiskimaged" },
      { cpu: 90, name: "mds_stores" },
    ],
    pause: async (ms) => {
      at += ms;
    },
    now: () => at,
    services: ["simdiskimaged", "mds_stores"],
    belowPercent: 20,
    quietSamples: 1,
    everyMs: 5_000,
    budgetMs: 10_000,
  });
  assert.equal(seen.settled, false);
});

test("adds up the service's processes when it runs several", async () => {
  const seen = await settle({
    sample: () => [
      { cpu: 15, name: "simdiskimaged" },
      { cpu: 15, name: "simdiskimaged" },
    ],
    pause: async () => undefined,
    now: (() => {
      let at = 0;
      return () => (at += 5_000);
    })(),
    services: ["simdiskimaged"],
    belowPercent: 20,
    quietSamples: 1,
    everyMs: 5_000,
    budgetMs: 10_000,
  });
  assert.equal(seen.settled, false);
});

test("says whether it settled, how long it took and what is still busy, as a notice", () => {
  assert.equal(
    said({
      settled: true,
      waitedMs: 15_000,
      services: ["simdiskimaged"],
      last: [{ cpu: 12.5, name: "mds_stores" }],
    }),
    "::notice title=Quieting the runner::simdiskimaged settled after 15 s. Busiest now: 12.5 mds_stores",
  );
  assert.equal(
    said({
      settled: false,
      waitedMs: 300_000,
      services: ["simdiskimaged"],
      last: [{ cpu: 96, name: "simdiskimaged" }],
    }),
    "::warning title=Quieting the runner::simdiskimaged did not settle in 300 s, so the bars measure on a busy runner. Busiest now: 96 simdiskimaged",
  );
});
