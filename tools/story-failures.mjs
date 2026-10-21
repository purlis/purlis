// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **Each story that failed, as a GitHub annotation** (#2901): its name, its file, and the first
// line of why, read from Vitest's JSON report. The reporter's own annotations name a source
// line, and a failure raised after the story ran (axe's, the screenshot's) has none to name.
//
//   node tools/story-failures.mjs <vitest json report>
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

/** A workflow command's property, escaped as the runner reads it. */
const property = (text) =>
  text
    .replace(/%/g, "%25")
    .replace(/\r/g, "%0D")
    .replace(/\n/g, "%0A")
    .replace(/:/g, "%3A")
    .replace(/,/g, "%2C");

/** Colour codes a terminal reporter leaves in a message. */
// eslint-disable-next-line no-control-regex
const ANSI = /\u001b\[[0-9;]*m/g;

/** A line of a stack trace, which says where, not why. */
const FRAME = /@https?:|@\[native code\]|^\s*at |^\s*❯/;

/** Each axe rule a message names, as the a11y addon prints it: `"<help> (<rule id>)"`. */
const RULE = /"[^"\n]*\(([a-z0-9-]+)\)"/g;

/** One `::error` line per failed test in a Vitest JSON report: the story's name, its file, the
 *  first line of why that is not a stack frame, and the axe rules it broke. */
export function annotations(report) {
  return report.testResults.flatMap((file) =>
    file.assertionResults
      .filter((one) => one.status === "failed")
      .map((one) => {
        const message = (one.failureMessages[0] ?? "").replace(ANSI, "");
        const lines = message.split("\n").map((line) => line.trim());
        const why =
          lines.find(
            (line) => line && !FRAME.test(line) && !/^Error:?$/.test(line),
          ) ?? "";
        const rules = [
          ...new Set([...message.matchAll(RULE)].map((found) => found[1])),
        ];
        const where = file.name.replace(/^.*\/app\//, "");
        const broke = rules.length > 0 ? ` [axe: ${rules.join(", ")}]` : "";
        return `::error title=${property(one.fullName)}::${where}: ${why.slice(0, 300)}${broke}`;
      }),
  );
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  for (const line of annotations(
    JSON.parse(readFileSync(process.argv[2], "utf8")),
  ))
    console.log(line);
}
