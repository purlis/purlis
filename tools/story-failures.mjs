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
//   It also names each story that takes no picture (`screenshot: false`), whose baselines
//   `tools/story-baselines.mjs` then calls obsolete;
// - run as a script, it reads those reports and prints one annotation per story that failed:
//
//   node tools/story-failures.mjs <browser> <report> [<browser> <report> ...]
//
// A story's annotation names the line of the story file it failed at, from the frames Vitest
// maps back to their sources, so it shows on the pull request's diff (`placeOf`).
//
// Every text an annotation carries is escaped as the runner reads a workflow command, so it
// stays one annotation on one line.
import { readFileSync, writeFileSync } from "node:fs";
import { isAbsolute, relative } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

/** The environment variable naming the file the reporter writes: an absolute path. */
export const REPORT = "PURLIS_STORY_REPORT";

/** A workflow command's data, escaped as the runner reads it (as `@actions/core` does). */
const data = (text) =>
  text.replace(/%/g, "%25").replace(/\r/g, "%0D").replace(/\n/g, "%0A");

/** A workflow command's property, escaped as the runner reads it. */
const property = (text) => data(text).replace(/:/g, "%3A").replace(/,/g, "%2C");

/** The longest reason an annotation gives, in characters. */
const LONGEST = 300;

/** Colour codes a terminal reporter leaves in a message. */
// eslint-disable-next-line no-control-regex
const ANSI = /\u001b\[[0-9;]*m/g;

/** Each axe rule a message names, as the a11y addon prints it: `"<help> (<rule id>)"`. */
const RULE = /"[^"\n]*\(([a-z0-9-]+)\)"/g;

/** A matcher's own line, `expect(received).toX()`, which says what was checked, not what was found. */
const MATCHER = /^expect\(.*\)\.[\w.]+\(.*\)$/;

/** The link to the story that Storybook's Vitest addon puts before a failed story's message. */
const DEBUG_LINK = /^Click to debug the error directly in Storybook:/;

/** React drawing a component: the frames of a story that failed to render, in either engine. */
const RENDERING =
  /\b(renderWithHooks|react_stack_bottom_frame|performUnitOfWork)\b/;

/** Storybook preparing a story, before it is drawn: its loaders and `beforeEach`. */
const PREPARING = /\b(applyLoaders|applyBeforeEach)\b/;

/** A frame of a story's own file, of its `play` function, or of the kit `play` functions call. */
const STORY_CODE =
  /\.stories\.tsx|stories\.testkit\.ts|(^|\n)\s*(at (async )?play \(|play@)/;

/**
 * **The gate a story failed** (`docs/component-stories.md`), told from its error alone, the same
 * way for both engines: the screenshot gate's setup file is on its stack, the a11y addon's
 * matcher is in its message, React's rendering or Storybook's preparing is on the stack of one
 * that did not render, and the story's own code is on the stack of a `play` function that failed.
 */
export function gateOf(error) {
  const message = error.message ?? "";
  const stack = error.stack ?? "";
  if (/screenshots\.setup\.ts/.test(stack) || /toMatchScreenshot/.test(message))
    return "screenshot";
  if (/toHaveNoViolations/.test(message) || /addon-a11y/.test(stack))
    return "accessibility";
  if (RENDERING.test(stack) || PREPARING.test(stack)) return "renders";
  if (STORY_CODE.test(stack)) return "keyboard";
  return "renders";
}

/** The first line of why: the message's first line, and the line after a matcher's own. */
export function whyOf(error) {
  const lines = (error.message ?? "")
    .replace(ANSI, "")
    .split(/\r\n|\r|\n/)
    .map((line) => line.trim())
    .filter(
      (line) => line && !/^Error:?$/.test(line) && !DEBUG_LINK.test(line),
    );
  if (lines.length === 0)
    return "the browser gave no message; the job's log has its stack";
  const [first, next] = lines;
  const why =
    MATCHER.test(first) && next ? `${first}: ${next.replace(/:$/, "")}` : first;
  // Cut by characters, not UTF-16 units, so no character is split.
  return [...why].slice(0, LONGEST).join("");
}

/**
 * **Where a story failed**: the first source-mapped frame Vitest gave with its error that is in a
 * story file, as the `file` (relative to the repository at `root`) and `line` of an annotation,
 * so it shows on the pull request's diff at the failing step. `undefined` when no frame is in a
 * story file inside `root`, such as axe's or the screenshot gate's. A frame in `own`, the failed
 * story's file as its report names it (relative to the app), comes before any other story file.
 */
export function placeOf(error, root, own) {
  const places = (error.frames ?? []).flatMap((frame) => {
    const path = String(frame.file ?? "")
      .replace(/^[a-z]+:\/\/[^/]*/i, "")
      .replace(/^\/@fs(?=\/)/, "")
      .replace(/[?#].*$/, "");
    if (!/\.stories\.tsx$/.test(path) || !isAbsolute(path)) return [];
    const file = relative(root, path).split("\\").join("/");
    const line = Number(frame.line);
    if (file.startsWith("../") || isAbsolute(file) || !(line >= 1)) return [];
    return [{ file, line: Math.trunc(line) }];
  });
  // The failed story's own file first, when the report names it; then any story file.
  const mine = own
    ? places.find(({ file }) => `/${file}`.endsWith(`/${own}`))
    : undefined;
  return mine ?? places[0];
}

/** One annotation: where it is, if anywhere, its title and its text, each escaped. */
const error = (title, text, at) =>
  `::error ${at ? `file=${property(at.file)},line=${at.line},` : ""}title=${property(title)}::${data(text)}`;

/** One `::error` line per failed story in one browser's report: the story's name and that
 *  browser, its file, its gate, why, and the axe rules it broke; one for each error outside any
 *  story; and, for a run that failed naming nothing, one that says so. The report does not say
 *  which browser ran it, so the job runs each browser on its own and says which report is whose. */
export function annotations(report, browser, root) {
  const failed = report.failures.map(({ file, story, cell, errors }) => {
    const [first = { message: "" }] = errors;
    const rules = [
      ...new Set(
        errors.flatMap((one) =>
          [...(one.message ?? "").matchAll(RULE)].map((found) => found[1]),
        ),
      ),
    ];
    const broke = rules.length > 0 ? ` [axe: ${rules.join(", ")}]` : "";
    return error(
      `${story} in ${browser}${cell ? `, ${cell}` : ""}`,
      `${file}: ${gateOf(first)}: ${whyOf(first)}${broke}`,
      root === undefined ? undefined : placeOf(first, root, file),
    );
  });
  const outside = report.errors.map((one) =>
    error(
      `stories in ${browser}`,
      `${one.file ? `${one.file}: ` : ""}an error outside any story: ${whyOf(one)}`,
    ),
  );
  const named = [...failed, ...outside];
  if (named.length === 0 && report.failed)
    return [
      error(
        `stories in ${browser}`,
        "the run failed but named no story; the job's log says why",
      ),
    ];
  return named;
}

/** An error as the report keeps it: its message and its stack, apart, and the frames Vitest
 *  mapped back to their sources. */
const kept = (one) => ({
  message: String(one.message ?? ""),
  stack: String(one.stack ?? ""),
  frames: (one.stacks ?? []).map((frame) => ({
    file: String(frame.file ?? ""),
    line: Number(frame.line),
    column: Number(frame.column),
  })),
});

/** Where the reporter writes: the absolute path `PURLIS_STORY_REPORT` names, or nowhere. */
export function reportPath(env) {
  const to = env[REPORT];
  return to && isAbsolute(to) ? to : null;
}

/** The cell of the theme matrix a story project runs in, from its name,
 *  `stories <cell> (<browser>)` (`app/.storybook/vitest.project.ts`); `undefined` for another. */
export function cellOf(project) {
  return /^stories (\S+) \(\w+\)$/.exec(project ?? "")?.[1];
}

/** **The reporter**: at the end of the run, each failed story's errors, the errors of a story
 *  file or a suite, those outside any, whether the run failed, and the stories that take no
 *  picture. */
export default class StoryFailures {
  onTestRunEnd(testModules, unhandledErrors = [], reason) {
    const failures = testModules.flatMap((module) =>
      [...module.children.allTests("failed")].map((test) => ({
        file: module.relativeModuleId,
        story: test.fullName.replaceAll(" > ", " "),
        cell: cellOf(module.project?.name),
        errors: (test.result().errors ?? []).map(kept),
      })),
    );
    const inFiles = testModules.flatMap((module) =>
      [module, ...module.children.allSuites()]
        .flatMap((part) => part.errors())
        .map((one) => ({ file: module.relativeModuleId, ...kept(one) })),
    );
    const errors = [...inFiles, ...unhandledErrors.map(kept)];
    // Each story that takes no picture (`screenshots.setup.ts` marks it), once, whatever cells
    // it ran in: its baselines are obsolete (`tools/story-baselines.mjs`).
    const unpictured = [
      ...new Map(
        testModules.flatMap((module) =>
          [...module.children.allTests()]
            .map((test) => test.meta())
            .map((meta) => meta.screenshot)
            .filter((shot) => shot?.taken === false && shot.story)
            .map(({ story }) => [
              `${module.relativeModuleId}\n${story}`,
              { file: module.relativeModuleId, id: String(story) },
            ]),
        ),
      ).values(),
    ];
    const failed =
      reason === "failed" ||
      reason === "interrupted" ||
      failures.length + errors.length > 0;
    const to = reportPath(process.env);
    if (to === null) {
      if (process.env[REPORT])
        console.error(
          `${REPORT} is not an absolute path; no report is written`,
        );
      return;
    }
    writeFileSync(
      to,
      JSON.stringify({ failed, failures, errors, unpictured }, null, 2),
    );
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  // `<browser> <report>`, once for each browser that ran.
  const given = process.argv.slice(2);
  // The repository, which an annotation's file is relative to.
  const root = process.env.GITHUB_WORKSPACE || process.cwd();
  for (let at = 0; at + 1 < given.length; at += 2) {
    let report;
    try {
      report = JSON.parse(readFileSync(given[at + 1], "utf8"));
    } catch {
      console.log(
        error(
          `stories in ${given[at]}`,
          "the run wrote no report; the job's log says why",
        ),
      );
      continue;
    }
    for (const line of annotations(report, given[at], root)) console.log(line);
  }
}
