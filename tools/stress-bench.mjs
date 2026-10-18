// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The `stress` job's relative rows (ADR 0086 §1, `docs/spec.md`), as github-action-benchmark's
// `customSmallerIsBetter` input (#818).
//
// Run by `stress.yml`'s `stress-record` job on a `main` run: `node tools/stress-bench.mjs
// <stress.jsonl> <os> <out.json>`. `app/e2e/specs/stress.e2e.ts` writes one line of JSON per look
// at the app into `app/logs/stress.jsonl`; this reads them back and writes each row with data as
// the median of that run's own samples, never one sample. A row with too few samples is left
// out with a warning, and with no row at all no file is written, so the job skips the tool
// rather than handing it an empty list it refuses.
//
// The rows:
// - **M2**, the web content process at fifty hot chats (the top class's hot target, ADR 0082
//   §3): one sample in each round's "fifty open" look. On macOS it is
//   `webContentFootprintPeakKb`, the footprint peak behind ADR 0082's 1,529 MB; resident memory
//   reads far low there. On Linux it is `webContentKb`, resident memory, since Linux has no
//   `footprint`. Two measures, so each OS is its own series and they are never compared (#818).
//   A macOS sample whose `footprintError` is set has no value, and is skipped, never read as 0.
// - **M3**, the native side with no chats: in every look with no harness running, the app's
//   resident memory as the window plus the session host it starts (`host` in each look since
//   #2597), the app as Activity Monitor shows it. ADR 0086 has M3 count the app and its daemon
//   together, and since #2085 the chats' terminals, threads and descriptors live in the host,
//   so the window alone no longer prices them (#2605). A look whose app runs no host
//   (`host: null`, or a record from before #2597) counts the window alone; a look whose host
//   has no reading is skipped, never read as the window alone. The row's name says "window +
//   session host", so the record starts a new series there rather than carrying on the
//   window-only one as if nothing had changed; the series up to then is "M3 native side with
//   no chats (resident)", the window alone. Each process's own median stays in `extra`.

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import process from "node:process";
import { fileURLToPath } from "node:url";

/** The fewest samples a row's median is taken of: one sample is not a median. */
export const AT_LEAST = 2;

/** The look each round of fifty takes with every tab open (`stress.e2e.ts`). */
const FIFTY_OPEN = "fifty open";

/** The records in a `stress.jsonl`, skipping any line that is not JSON (a write cut short). */
export function readSamples(text) {
  const samples = [];
  for (const line of text.split("\n")) {
    if (!line.trim()) continue;
    try {
      const record = JSON.parse(line);
      if (record && typeof record === "object") samples.push(record);
    } catch {
      // A line the run was killed in the middle of.
    }
  }
  return samples;
}

/** The middle of `values`, or the mean of the two middle ones. */
export function median(values) {
  const sorted = [...values].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 1
    ? sorted[mid]
    : (sorted[mid - 1] + sorted[mid]) / 2;
}

const kilobytes = (value) =>
  typeof value === "number" && Number.isFinite(value) && value > 0
    ? value
    : null;

/** Whether `os`, a runner label such as `macos-latest`, is macOS. */
const isMac = (os) => /^macos/i.test(os);

/** Each row's samples on `os`, in kilobytes, with what the row is called and measures. */
function rowsOf(samples, os) {
  const mac = isMac(os);
  const fifty = samples.filter((s) => s.step === FIFTY_OPEN);
  const webContent = mac
    ? fifty
        .filter((s) => s.footprintError == null)
        .map((s) => kilobytes(s.webContentFootprintPeakKb))
    : fifty.map((s) => kilobytes(s.webContentKb));
  const noChats = samples.filter((s) => s.harnesses === 0).flatMap(theApp);
  return [
    {
      name: `M2 web content at fifty hot chats (${mac ? "footprint peak" : "resident"})`,
      values: webContent.filter((v) => v !== null),
    },
    {
      name: "M3 native side with no chats (window + session host, resident)",
      values: noChats.map((app) => app.total),
      parts: [
        ["window", noChats.map((app) => app.window)],
        ["session host", noChats.flatMap((app) => (app.host === null ? [] : [app.host]))],
      ],
    },
  ];
}

/**
 * The app in one look, in kilobytes: the window, its session host (`null` when it runs none),
 * and the two together. No entry when either process has no reading.
 */
function theApp(look) {
  const window = kilobytes(look.rssKb);
  if (window === null) return [];
  if (look.host == null) return [{ window, host: null, total: window }];
  const host = kilobytes(look.host.rssKb);
  if (host === null) return [];
  return [{ window, host, total: window + host }];
}

const megabytes = (kb) => Math.round((kb / 1024) * 10) / 10;

/**
 * The benchmark entries for `os`, and a warning for each row left out. Each entry is the
 * median of its samples in MB, with the samples themselves in `extra`.
 */
export function benchmarks(samples, os) {
  const entries = [];
  const warnings = [];
  for (const row of rowsOf(samples, os)) {
    if (row.values.length < AT_LEAST) {
      warnings.push(
        `${row.name} (${os}): ${row.values.length} sample(s), and a median needs ${AT_LEAST}; not recorded`,
      );
      continue;
    }
    entries.push({
      name: row.name,
      unit: "MB",
      value: megabytes(median(row.values)),
      extra: `median of ${row.values.length} samples: ${row.values.map(megabytes).join(", ")} MB${partsOf(row)}`,
    });
  }
  return { entries, warnings };
}

/** Each process's own median beside a row that sums them, such as `; window 197.3 MB, session host 58.6 MB`. */
function partsOf(row) {
  if (!row.parts) return "";
  const each = row.parts.map(([name, values]) =>
    values.length === 0 ? `${name} none` : `${name} ${megabytes(median(values))} MB`,
  );
  return `; ${each.join(", ")}`;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const [file, os, out] = process.argv.slice(2);
  if (!file || !os || !out) {
    console.error("usage: node tools/stress-bench.mjs <stress.jsonl> <os> <out.json>");
    process.exit(2);
  }
  const samples = readSamples(existsSync(file) ? readFileSync(file, "utf8") : "");
  const { entries, warnings } = benchmarks(samples, os);
  for (const warning of warnings) console.log(`::warning title=stress record::${warning}`);
  if (entries.length > 0) {
    writeFileSync(out, `${JSON.stringify(entries, null, 2)}\n`);
  }
  console.log(`${entries.length} row(s) recorded from ${samples.length} sample(s) on ${os}`);
}
