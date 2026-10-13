// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// What the scenario run's project switches took, as one GitHub notice.
//
// Run by the `scenario` jobs after the specs: `node ../tools/switch-notice.mjs
// logs/project-switch.jsonl "<label>"` from `app/`. `specs/project-switch.e2e.ts` writes one line
// of JSON there: thirty switches among ten projects, timed inside the window. A notice is read
// through the checks API, the one place a run's numbers are visible without its log.
//
// The switches are taken a round at a time, and the first of each round is the switch to the
// launch's project, which holds the chats earlier specs left in it where the rest hold one. So
// the notice says the two apart: what a switch to a project of many chats took, and what a
// switch to a project of one did. Evidence, never a verdict (ADR 0086): the job's part that ran
// no switch spec says so and nothing else.

import { existsSync, readFileSync } from "node:fs";
import process from "node:process";
import { fileURLToPath } from "node:url";

/** The last record in a `project-switch.jsonl`, skipping any line that is not JSON. */
export function readSwitches(text) {
  let last;
  for (const line of text.split("\n")) {
    if (!line.trim()) continue;
    try {
      last = JSON.parse(line);
    } catch {
      continue;
    }
  }
  return last;
}

// Workflow command escaping: `%`, CR and LF in data; `:` and `,` too in a property.
const escapeData = (s) =>
  s.replace(/%/g, "%25").replace(/\r/g, "%0D").replace(/\n/g, "%0A");
const escapeProperty = (s) =>
  escapeData(s).replace(/:/g, "%3A").replace(/,/g, "%2C");

const ms = (n) => `${Math.round(n)} ms`;
const middle = (samples) =>
  [...samples].sort((a, b) => a - b)[Math.floor(samples.length / 2)];
const worst = (samples) => Math.max(...samples);
const each = (samples) => samples.map((one) => Math.round(one)).join(", ");

/** The `::notice` line for `record`, titled with `label`; none where no switch was recorded. */
export function annotation(record, label) {
  if (!record || !Array.isArray(record.samples_ms) || !record.projects)
    return undefined;
  const { projects, samples_ms: painted, shown_ms: shown = [] } = record;
  // A round is one switch to each project, the launch's first.
  const toLaunch = (samples) => samples.filter((_, at) => at % projects === 0);
  const toRest = (samples) => samples.filter((_, at) => at % projects !== 0);
  const chats =
    record.launchChats === undefined ? "its" : `${record.launchChats}`;
  const lines = [
    `To the launch's project (${chats} chats), press to its pane painted: ${each(toLaunch(painted))} ms`,
    `To a project of one chat, press to its pane painted: median ${ms(middle(toRest(painted)))}, worst ${ms(worst(toRest(painted)))}`,
  ];
  if (shown.length === painted.length)
    lines.push(
      `Press to the project in front, before its pane: launch's ${each(toLaunch(shown))} ms; one chat median ${ms(middle(toRest(shown)))}, worst ${ms(worst(toRest(shown)))}`,
    );
  lines.push(
    `All ${painted.length} switches among ${projects} projects: p50 ${ms(record.p50)}, p95 ${ms(record.p95)}, worst ${ms(record.worst)}`,
  );
  return `::notice title=${escapeProperty(`Project switches (${label})`)}::${escapeData(lines.join("\n"))}`;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const file = process.argv[2] ?? "logs/project-switch.jsonl";
  const label = process.argv[3] ?? "scenario";
  const line = annotation(
    readSwitches(existsSync(file) ? readFileSync(file, "utf8") : ""),
    label,
  );
  if (line) console.log(line);
}
