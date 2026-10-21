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

/** One `::error` line per failed test in a Vitest JSON report. */
export function annotations(report) {
  return report.testResults.flatMap((file) =>
    file.assertionResults
      .filter((one) => one.status === "failed")
      .map((one) => {
        const why =
          (one.failureMessages[0] ?? "")
            .split("\n")
            .find((line) => line.trim()) ?? "";
        const where = file.name.replace(/^.*\/app\//, "");
        return `::error title=${property(one.fullName)}::${where}: ${why.trim().slice(0, 300)}`;
      }),
  );
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  for (const line of annotations(
    JSON.parse(readFileSync(process.argv[2], "utf8")),
  ))
    console.log(line);
}
