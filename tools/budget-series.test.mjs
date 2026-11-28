// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The budget run's results as `macos-max` series points (#7737). Run with `node --test tools/`.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

import { PINNED_SCREEN, screenMode, seriesPoints } from "./budget-series.mjs";

/** One arm's `burst` spec as `tools/bench.mjs --only window` keeps it in `results.json`. */
const burst = (light, heavy) => ({
  "tab switch to a light tab": { samples: light.length, p50: [...light].sort((a, b) => a - b)[Math.floor(light.length / 2)], worst: Math.max(...light), samples_ms: light },
  "tab switch to a tab with 5000 lines of history": {
    samples: heavy.length,
    p50: [...heavy].sort((a, b) => a - b)[Math.floor(heavy.length / 2)],
    worst: Math.max(...heavy),
    samples_ms: heavy,
    historyLinesWritten: 6000,
  },
});

const results = {
  machine: { os: "macOS 15.6.1", cpu: "Apple M1 Max", memoryGb: 64, charterApp: "7a54a90" },
  window: {
    dom: { burst: burst([12, 14, 13, 30, 11], [40, 42, 41, 39, 90]) },
    webgl: { burst: burst([9, 10, 8, 11, 9], [20, 21, 22, 19, 25]) },
  },
};

const machine = { commit: "7a54a9065", model: "Mac13,1", os: "macOS 15.6.1", screen: PINNED_SCREEN };

/** `system_profiler SPDisplaysDataType` on an M1 Max with one display, as macOS prints it. */
const displays = (resolution, looksLike) => `Graphics/Displays:

    Apple M1 Max:

      Chipset Model: Apple M1 Max
      Type: GPU
      Bus: Built-In
      Total Number of Cores: 32
      Vendor: Apple (0x106b)
      Metal Support: Metal 3
      Displays:
        Dummy:
          Resolution: ${resolution}
${looksLike ? `          UI Looks like: ${looksLike}\n` : ""}          Main Display: Yes
          Mirror: Off
          Online: Yes
`;

test("each row and arm is a series of its own, named by the row, the case and the arm", () => {
  const { series } = seriesPoints(results, machine);
  assert.deepEqual(
    series.map((p) => p.name),
    [
      "L2 tab switch to a light tab (dom)",
      "L2 tab switch to a tab with 5000 lines of history (dom)",
      "L2 tab switch to a light tab (webgl)",
      "L2 tab switch to a tab with 5000 lines of history (webgl)",
    ],
  );
});

test("a point is the median of the run's switches, in ms, as github-action-benchmark reads it", () => {
  const { series } = seriesPoints(results, machine);
  const light = series.find((p) => p.name === "L2 tab switch to a light tab (dom)");
  assert.equal(light.unit, "ms");
  assert.equal(light.value, 13);
  assert.match(light.extra, /median of 5 switches: 12, 14, 13, 30, 11 ms; worst 30 ms/);
});

test("every point carries the commit, the Mac's model, its OS and its screen mode", () => {
  for (const point of seriesPoints(results, machine).series) {
    assert.match(point.extra, /commit 7a54a9065/);
    assert.match(point.extra, /Mac13,1/);
    assert.match(point.extra, /macOS 15\.6\.1/);
    assert.match(point.extra, new RegExp(`screen ${PINNED_SCREEN}`));
  }
});

test("a run in any screen mode but the pinned one is evidence only, never a series point", () => {
  const other = { ...machine, screen: "1728x1117 HiDPI @ 120 Hz" };
  const { series, evidence, warnings } = seriesPoints(results, other);
  assert.deepEqual(series, []);
  assert.equal(evidence.length, 4);
  assert.match(evidence[0].extra, /screen 1728x1117 HiDPI @ 120 Hz/);
  assert.match(warnings.join("\n"), /not the pinned 1920x1080 HiDPI @ 60 Hz.*evidence only/);
});

test("a screen that only nearly matches the pinned one is evidence only too", () => {
  // Each is one way a looser match than the pinned mode, whole, would pass it into the trend.
  for (const screen of [
    "1920x1080 @ 60 Hz", // not HiDPI: the dummy plug's mode without its scaling
    "1920x1080 HiDPI @ 59.94 Hz", // the NTSC rate, not the pinned 60 Hz
    `${PINNED_SCREEN} + 1920x1080`, // a second display beside the pinned one
    "no display",
    undefined, // no screen read at all
  ]) {
    const { series, evidence } = seriesPoints(results, { ...machine, screen });
    assert.deepEqual(series, [], String(screen));
    assert.equal(evidence.length, 4, String(screen));
  }
});

test("a row that recorded nothing, or one switch, is left out with a warning", () => {
  const thin = {
    machine: results.machine,
    window: {
      dom: { "burst.bench.ts failed": "Error: wdio failed: 1" },
      webgl: { burst: { "tab switch to a light tab": { samples: 1, p50: 9, worst: 9, samples_ms: [9] } } },
    },
  };
  const { series, warnings } = seriesPoints(thin, machine);
  assert.deepEqual(series, []);
  assert.equal(warnings.length, 4);
  assert.match(warnings.join("\n"), /L2 tab switch to a light tab \(webgl\): 1 switch\(es\)/);
  assert.match(warnings.join("\n"), /L2 tab switch to a light tab \(dom\): not recorded/);
});

test("the screen mode is what the display looks like, HiDPI when it draws more pixels, and its rate", () => {
  assert.equal(screenMode(displays("3840 x 2160 (2160p/4K UHD - Ultra High Definition)", "1920 x 1080 @ 60.00Hz")), PINNED_SCREEN);
  assert.equal(screenMode(displays("1920 x 1080 (1080p FHD - Full High Definition)", "1920 x 1080 @ 60.00Hz")), "1920x1080 @ 60 Hz");
  assert.equal(screenMode(displays("3456 x 2234 Retina", "1728 x 1117 @ 120.00Hz")), "1728x1117 HiDPI @ 120 Hz");
  assert.equal(screenMode(displays("2560 x 1440", null)), "2560x1440");
  // **59.94 Hz is not the pinned 60 Hz** (#7944): a rate is rounded to its hundredths, never to a
  // whole hertz, so a Mac that came up in the NTSC mode reads as another screen and is evidence.
  assert.equal(screenMode(displays("3840 x 2160", "1920 x 1080 @ 59.94Hz")), "1920x1080 HiDPI @ 59.94 Hz");
  assert.equal(screenMode(displays("3840 x 2160", "1920 x 1080 @ 60.00Hz")), PINNED_SCREEN);
  assert.equal(screenMode("Graphics/Displays:\n\n    Apple M1 Max:\n\n      Chipset Model: Apple M1 Max\n"), "no display");
  const two = displays("3840 x 2160", "1920 x 1080 @ 60.00Hz") + "        Other:\n          Resolution: 1920 x 1080\n";
  assert.equal(screenMode(two), `${PINNED_SCREEN} + 1920x1080`);
});

test("the command writes the series and the evidence apart, and no file for an empty set", () => {
  const dir = mkdtempSync(join(tmpdir(), "budget-series-"));
  const script = fileURLToPath(new URL("./budget-series.mjs", import.meta.url));
  const input = join(dir, "results.json");
  const shown = join(dir, "displays.txt");
  writeFileSync(input, JSON.stringify(results));
  writeFileSync(shown, displays("3840 x 2160", "1920 x 1080 @ 60.00Hz"));
  const out = join(dir, "series.json");
  const evidence = join(dir, "evidence.json");
  const done = spawnSync(
    process.execPath,
    [script, "--results", input, "--displays", shown, "--model", "Mac13,1", "--commit", "7a54a9065", "--out", out, "--evidence-out", evidence],
    { encoding: "utf8", env: { ...process.env, GITHUB_STEP_SUMMARY: join(dir, "summary.md") } },
  );
  assert.equal(done.status, 0, done.stderr);
  assert.match(readFileSync(join(dir, "summary.md"), "utf8"), /4 series point\(s\), 0 evidence point\(s\); screen 1920x1080 HiDPI @ 60 Hz/);
  assert.equal(JSON.parse(readFileSync(out, "utf8")).length, 4);
  assert.equal(existsSync(evidence), false);
  assert.match(done.stdout, /4 series point\(s\), 0 evidence point\(s\)/);
  const point = JSON.parse(readFileSync(out, "utf8"))[0];
  // The OS comes from the bench's own record of the machine.
  assert.match(point.extra, /macOS 15\.6\.1/);

  // Anything but the pinned screen: the points go to the evidence alone, and no series is written.
  const otherOut = join(dir, "other-series.json");
  const otherEvidence = join(dir, "other-evidence.json");
  writeFileSync(shown, displays("1920 x 1080 (1080p FHD - Full High Definition)", "1920 x 1080 @ 60.00Hz"));
  const other = spawnSync(
    process.execPath,
    [script, "--results", input, "--displays", shown, "--model", "Mac13,1", "--commit", "7a54a9065", "--out", otherOut, "--evidence-out", otherEvidence],
    { encoding: "utf8", env: { ...process.env, GITHUB_STEP_SUMMARY: join(dir, "other-summary.md") } },
  );
  assert.equal(other.status, 0, other.stderr);
  assert.equal(existsSync(otherOut), false);
  const kept = JSON.parse(readFileSync(otherEvidence, "utf8"));
  assert.equal(kept.length, 4);
  assert.match(kept[0].extra, /screen 1920x1080 @ 60 Hz/);
  assert.match(other.stdout, /0 series point\(s\), 4 evidence point\(s\); screen 1920x1080 @ 60 Hz/);
  assert.match(readFileSync(join(dir, "other-summary.md"), "utf8"), /recorded as evidence only/);

  const missing = spawnSync(process.execPath, [script, "--results", join(dir, "none.json"), "--displays", shown, "--model", "m", "--commit", "c", "--out", join(dir, "x.json"), "--evidence-out", join(dir, "y.json")], { encoding: "utf8" });
  assert.equal(missing.status, 0, missing.stderr);
  assert.equal(existsSync(join(dir, "x.json")), false);
  assert.match(missing.stdout, /::warning title=budget run::/);
});
