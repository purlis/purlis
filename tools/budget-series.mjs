// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The budget run's results as `macos-max` series points (#7737, spec #7730): github-action-
// benchmark's `customSmallerIsBetter` input, one series per budget row, case and renderer arm.
//
// Run by `budget-run.yml` on the benchmark Mac, after `tools/bench.mjs --only window` wrote its
// `results.json`:
//
//   node tools/budget-series.mjs --results <results.json> --displays <system_profiler.txt> \
//     --model "$(sysctl -n hw.model)" --commit "$GITHUB_SHA" --out <series.json> \
//     --evidence-out <evidence.json>
//
// Each point is the median of the run's own samples, never one sample, and carries the commit,
// the Mac's model, its OS and its screen mode in `extra`, so a hardware or OS change reads as a
// visible step and not as a regression. **A run whose screen mode is not the pinned one is
// evidence only** (#7730): its points go to `--evidence-out`, a data set apart, so the trend never
// mixes two screen setups. A row with too few samples is left out with a warning, and an empty set
// writes no file, so the job skips the record step rather than handing the tool an empty list.
//
// The rows are `ROWS` below: each names the window bench spec that measures it and the case it
// records under. The tracer bullet holds L2; the nightly set adds its rows here.

import { appendFileSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

import { AT_LEAST, median } from "./stress-bench.mjs";

/**
 * The benchmark Mac's screen, as the mac-farm bootstrap pins it on its HDMI dummy plug: "looks like
 * 1920×1080", HiDPI, at 60 Hz (#7730). In `screenMode`'s spelling, and matched whole: a 59.94 Hz
 * mode, a second display or no HiDPI is another screen setup.
 */
export const PINNED_SCREEN = "1920x1080 HiDPI @ 60 Hz";

/**
 * The budget rows the run records: the row, the bench spec (`app/e2e/bench/<spec>.bench.ts`, the
 * name it records under) and the case inside it.
 */
export const ROWS = [
  { row: "L2", spec: "burst", case: "tab switch to a light tab" },
  { row: "L2", spec: "burst", case: "tab switch to a tab with 5000 lines of history" },
];

const round = (ms) => Math.round(ms * 10) / 10;

/**
 * The points of one run.
 *
 * @param results `tools/bench.mjs`'s `results.json`: `window.<arm>.<spec>.<case>.samples_ms`.
 * @param machine `{ commit, model, os, screen }`; `os` falls back to the bench's own record.
 * @returns `{ series, evidence, warnings }`: the points of a pinned-screen run in `series`, of
 *   any other in `evidence`, and one warning per row left out.
 */
export function seriesPoints(results, machine) {
  const os = machine.os ?? results?.machine?.os ?? "unknown OS";
  const pinned = machine.screen === PINNED_SCREEN;
  const points = [];
  const warnings = [];
  for (const [arm, specs] of Object.entries(results?.window ?? {})) {
    for (const row of ROWS) {
      const name = `${row.row} ${row.case} (${arm})`;
      const samples = specs?.[row.spec]?.[row.case]?.samples_ms;
      if (!Array.isArray(samples)) {
        warnings.push(`${name}: not recorded (${failureOf(specs, row.spec)})`);
        continue;
      }
      const ms = samples.filter((v) => typeof v === "number" && Number.isFinite(v));
      if (ms.length < AT_LEAST) {
        warnings.push(`${name}: ${ms.length} switch(es), and a median needs ${AT_LEAST}; not recorded`);
        continue;
      }
      points.push({
        name,
        unit: "ms",
        value: round(median(ms)),
        extra:
          `median of ${ms.length} switches: ${ms.map(round).join(", ")} ms; worst ${round(Math.max(...ms))} ms; ` +
          `commit ${machine.commit}, ${machine.model}, ${os}, screen ${machine.screen}`,
      });
    }
  }
  if (!pinned && points.length > 0)
    warnings.push(`the screen is ${machine.screen}, not the pinned ${PINNED_SCREEN}: this run's points are evidence only`);
  return { series: pinned ? points : [], evidence: pinned ? [] : points, warnings };
}

function failureOf(specs, spec) {
  const failed = specs?.[`${spec}.bench.ts failed`] ?? specs?.[`${spec}.bench.ts skipped`];
  return failed ? `${spec}.bench.ts: ${String(failed).split("\n")[0]}` : `${spec}.bench.ts recorded no such case`;
}

/**
 * The screen mode of `system_profiler SPDisplaysDataType`'s text: per display, the size it looks
 * like, `HiDPI` when it draws more pixels than that, and its refresh rate, such as
 * `1920x1080 HiDPI @ 60 Hz`; several displays joined by ` + `, and `no display` for none.
 */
export function screenMode(text) {
  const modes = [];
  let display = null;
  const done = () => {
    if (display) modes.push(display);
    display = null;
  };
  for (const line of String(text).split("\n")) {
    const resolution = /^\s*Resolution:\s*(\d+)\s*x\s*(\d+)(.*)$/.exec(line);
    if (resolution) {
      done();
      display = { width: +resolution[1], height: +resolution[2], retina: /Retina/.test(resolution[3]) };
      continue;
    }
    const looks = /^\s*UI Looks like:\s*(\d+)\s*x\s*(\d+)(?:\s*@\s*([\d.]+)\s*Hz)?/.exec(line);
    if (looks && display) Object.assign(display, { looksWidth: +looks[1], looksHeight: +looks[2], hz: looks[3] });
  }
  done();
  if (modes.length === 0) return "no display";
  return modes
    .map((d) => {
      const width = d.looksWidth ?? d.width;
      const height = d.looksHeight ?? d.height;
      const hidpi = d.retina || d.width > width;
      return `${width}x${height}${hidpi ? " HiDPI" : ""}${d.hz ? ` @ ${hertz(d.hz)} Hz` : ""}`;
    })
    .join(" + ");
}

/**
 * A refresh rate to its hundredths, `60.00` as `60` and `59.94` as `59.94`: never rounded to a
 * whole hertz, so the NTSC rate is not taken for the pinned 60 Hz (#7944). A Mac that came up at
 * 59.94 Hz is another screen setup, and its points are evidence.
 */
const hertz = (hz) => String(Math.round(Number(hz) * 100) / 100);

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const { values: o } = parseArgs({
    options: {
      results: { type: "string" },
      displays: { type: "string" },
      model: { type: "string" },
      commit: { type: "string" },
      out: { type: "string" },
      "evidence-out": { type: "string" },
    },
  });
  if (!o.results || !o.displays || !o.model || !o.commit || !o.out || !o["evidence-out"]) {
    console.error(
      "usage: node tools/budget-series.mjs --results <results.json> --displays <file> --model <hw.model> " +
        "--commit <sha> --out <series.json> --evidence-out <evidence.json>",
    );
    process.exit(2);
  }
  const results = existsSync(o.results) ? JSON.parse(readFileSync(o.results, "utf8")) : {};
  if (!existsSync(o.results)) console.log(`::warning title=budget run::${o.results} was not written: the bench ran no window spec`);
  const screen = screenMode(existsSync(o.displays) ? readFileSync(o.displays, "utf8") : "");
  const { series, evidence, warnings } = seriesPoints(results, { commit: o.commit, model: o.model, screen });
  for (const warning of warnings) console.log(`::warning title=budget run::${warning}`);
  if (series.length > 0) writeFileSync(o.out, `${JSON.stringify(series, null, 2)}\n`);
  if (evidence.length > 0) writeFileSync(o["evidence-out"], `${JSON.stringify(evidence, null, 2)}\n`);
  const said = `${series.length} series point(s), ${evidence.length} evidence point(s); screen ${screen}`;
  console.log(said);
  if (process.env.GITHUB_STEP_SUMMARY)
    appendFileSync(
      process.env.GITHUB_STEP_SUMMARY,
      `### The budget run's points\n\n${said} (pinned: ${PINNED_SCREEN}).` +
        `${evidence.length > 0 ? " Not the pinned screen, so they are recorded as evidence only." : ""}\n` +
        warnings.map((w) => `\n- ${w}`).join("") + "\n",
    );
}
