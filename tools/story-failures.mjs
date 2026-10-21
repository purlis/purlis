// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **Each story that failed, as a GitHub annotation** (#2901, #2944): its name and browser, its
// file, the gate it failed, and the first line of why, with the axe rules it broke.
//
// Two halves, one file:
//
// - the default export is a Vitest reporter, which writes each failed story's errors to the
//   file `PURLIS_STORY_REPORT` names. It keeps an error's message apart from its stack: WebKit's
//   stack does not repeat the message, as Chromium's does, so a report that keeps only the
//   stack (Vitest's JSON one) loses why a story failed in WebKit;
// - run as a script, it reads those reports and prints one annotation per story that failed:
//
//   node tools/story-failures.mjs <browser> <report> [<browser> <report> ...]
import { readFileSync, writeFileSync } from "node:fs";
import process from "node:process";
import { fileURLToPath } from "node:url";

/** The environment variable naming the file the reporter writes. */
export const REPORT = "PURLIS_STORY_REPORT";

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

/** Each axe rule a message names, as the a11y addon prints it: `"<help> (<rule id>)"`. */
const RULE = /"[^"\n]*\(([a-z0-9-]+)\)"/g;

/** A matcher's own line, `expect(received).toX()`, which says what was checked, not what was found. */
const MATCHER = /^expect\(.*\)\.[\w.]+\(.*\)$/;

/** React drawing a component: the frames of a story that failed to render, in either engine. */
const RENDERING =
  /\b(renderWithHooks|react_stack_bottom_frame|performUnitOfWork)\b/;

/** A frame of a story's own file, or of the kit its `play` functions call. */
const STORY_CODE = /\.stories\.tsx|stories\.testkit\.ts|\bplay\b/;

/**
 * **The gate a story failed** (`docs/component-stories.md`), told from its error alone, the same
 * way for both engines: the screenshot gate's setup file is on its stack, the a11y addon's
 * matcher is in its message, React's rendering is on the stack of one that did not render, and
 * the story's own code is on the stack of a `play` function that failed.
 */
export function gateOf(error) {
  const message = error.message ?? "";
  const stack = error.stack ?? "";
  if (/screenshots\.setup\.ts/.test(stack) || /toMatchScreenshot/.test(message))
    return "screenshot";
  if (/toHaveNoViolations/.test(message) || /addon-a11y/.test(stack))
    return "accessibility";
  if (RENDERING.test(stack)) return "renders";
  if (STORY_CODE.test(stack)) return "keyboard";
  return "renders";
}

/** The first line of why: the message's first line, and the line after a matcher's own. */
export function whyOf(error) {
  const lines = (error.message ?? "")
    .replace(ANSI, "")
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line && !/^Error:?$/.test(line));
  if (lines.length === 0)
    return "the browser gave no message; the job's log has its stack";
  const [first, next] = lines;
  const why =
    MATCHER.test(first) && next ? `${first}: ${next.replace(/:$/, "")}` : first;
  return why.slice(0, 300);
}

/** One `::error` line per failed story in one browser's report: the story's name and that
 *  browser, its file, its gate, why, and the axe rules it broke. The report does not say which
 *  browser ran it, so the job runs each browser on its own and says which report is whose. */
export function annotations(report, browser) {
  const failed = report.failures.map(({ file, story, errors }) => {
    const [error = { message: "" }] = errors;
    const rules = [
      ...new Set(
        errors.flatMap((one) =>
          [...(one.message ?? "").matchAll(RULE)].map((found) => found[1]),
        ),
      ),
    ];
    const broke = rules.length > 0 ? ` [axe: ${rules.join(", ")}]` : "";
    return `::error title=${property(`${story} in ${browser}`)}::${file}: ${gateOf(error)}: ${whyOf(error)}${broke}`;
  });
  const outside = report.errors.map(
    (error) =>
      `::error title=${property(`stories in ${browser}`)}::an error outside any story: ${whyOf(error)}`,
  );
  return [...failed, ...outside];
}

/** An error as the report keeps it: its message and its stack, apart. */
const kept = (error) => ({
  message: String(error.message ?? ""),
  stack: String(error.stack ?? ""),
});

/** **The reporter**: at the end of the run, each failed story's errors, and those outside any. */
export default class StoryFailures {
  onTestRunEnd(testModules, unhandledErrors = []) {
    const failures = testModules.flatMap((module) =>
      [...module.children.allTests("failed")].map((test) => ({
        file: module.relativeModuleId,
        story: test.fullName.replaceAll(" > ", " "),
        errors: (test.result().errors ?? []).map(kept),
      })),
    );
    const to = process.env[REPORT];
    if (to)
      writeFileSync(
        to,
        JSON.stringify(
          { failures, errors: unhandledErrors.map(kept) },
          null,
          2,
        ),
      );
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  // `<browser> <report>`, once for each browser that ran.
  const given = process.argv.slice(2);
  for (let at = 0; at + 1 < given.length; at += 2) {
    let report;
    try {
      report = JSON.parse(readFileSync(given[at + 1], "utf8"));
    } catch {
      console.log(
        `::error title=stories in ${given[at]}::the run wrote no report; the job's log says why`,
      );
      continue;
    }
    for (const line of annotations(report, given[at])) console.log(line);
  }
}
