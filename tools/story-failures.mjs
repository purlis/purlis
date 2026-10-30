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
// - run as a script, it reads those reports and prints one annotation per story that failed,
//   those that stop the baselines being drawn first, and writes the same to the step's summary,
//   which GitHub never caps (`named`, #4050):
//
//   node tools/story-failures.mjs <browser> <report> [<browser> <report> ...]
//
//   Or, for the check, the one annotation that names every story that stops them, and every
//   shard that wrote no report, from the directory of every shard's report:
//
//   node tools/story-failures.mjs headline <reports>
//
//   Or, to ask whether a picture is all that failed in the reports (`picturesAlone`), which is
//   when the stories job draws the baselines its pull request can take (`stories.yml`); it
//   prints the story files whose pictures failed, one a line, the ones to draw again:
//
//   node tools/story-failures.mjs pictures-alone <report> [<report> ...]
//
//   The stories run in shards (#4013), each a browser and a part of its story files. A shard
//   whose pictures alone failed asks which to draw again: each story file and cell whose
//   picture failed, `<file>\t<cell>` a line (`picturesToDraw`), and nothing, failing, otherwise:
//
//   node tools/story-failures.mjs pictures-to-draw <report>
//
//   And the check asks whether every shard reported (`shardProblems`) and, given the directory
//   the shards' drawings were put in, whether each shard that failed drew (`undrawn`); it prints
//   each problem and fails on any:
//
//   node tools/story-failures.mjs shards <reports> [<drawn>]
//
// A story's annotation names the line of the story file it failed at, from the frames Vitest
// maps back to their sources, so it shows on the pull request's diff (`placeOf`).
//
// Every text an annotation carries is escaped as the runner reads a workflow command, so it
// stays one annotation on one line.
import { appendFileSync, existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { isAbsolute, join, relative } from "node:path";
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

/** Whether a failed story stops its run drawing baselines: it failed on more than its picture,
 *  or with no error at all to say which gate. */
export function blocksBaselines({ errors }) {
  return errors.length === 0 || errors.some((one) => gateOf(one) !== "screenshot");
}

/**
 * **Whether a picture is all that failed** in these reports, one per browser: a story failed,
 * every error of every story that failed is the screenshot gate's, no error is outside a story,
 * and no run failed without naming a story. Only then are a run's baselines drawn for its pull
 * request to take (`stories.yml`): a baseline is drawn only of a story that passes everything
 * else.
 */
export function picturesAlone(reports) {
  const failures = reports.flatMap((report) => report.failures ?? []);
  return (
    failures.length > 0 &&
    reports.every(
      (report) =>
        (report.errors ?? []).length === 0 &&
        (!report.failed || (report.failures ?? []).length > 0),
    ) &&
    failures.every((failure) => !blocksBaselines(failure))
  );
}

/** The browsers the stories run in, each in its shards (`stories.yml`). */
export const BROWSERS = ["chromium", "webkit"];

/**
 * **The pictures a shard draws again**: each story file and cell whose picture failed in its
 * `report`, once each, when a picture is all that failed there (`picturesAlone`); `undefined`
 * otherwise, or when a failure names no cell. A story file's cells can fall in different shards
 * (Vitest shards a file in each project apart), so a shard keeps only its own cells' pictures, and
 * no two shards keep the same one.
 */
export function picturesToDraw(report) {
  if (!picturesAlone([report])) return undefined;
  const pairs = new Map();
  for (const { file, cell } of report.failures) {
    if (typeof cell !== "string" || cell === "") return undefined;
    pairs.set(`${file}\n${cell}`, { file, cell });
  }
  return [...pairs.values()];
}

/** A shard's report's file name, `report-<browser>-<shard>-of-<shards>.json`, read; `undefined`
 *  for any other name. */
export function shardOf(fileName) {
  const found = /^report-([a-z]+)-(\d+)-of-(\d+)\.json$/.exec(fileName);
  if (!found) return undefined;
  const [, browser, shard, of] = found;
  if (Number(shard) < 1 || Number(shard) > Number(of)) return undefined;
  return { browser, shard: Number(shard), of: Number(of), name: `${browser}-${shard}-of-${of}` };
}

/** What is missing of the shards' reports, by their file names: every browser in `BROWSERS`
 *  wrote one from each shard, and its shards agree on how many there are. */
export function shardProblems(fileNames) {
  const shards = fileNames.map(shardOf).filter(Boolean);
  return BROWSERS.flatMap((browser) => {
    const its = shards.filter((one) => one.browser === browser);
    if (its.length === 0) return [`${browser}: no shard wrote a report`];
    const counts = [...new Set(its.map(({ of }) => of))].sort((a, b) => a - b);
    if (counts.length > 1)
      return [`${browser}: its shards disagree on how many there are (${counts.join(", ")})`];
    const [of] = counts;
    const have = new Set(its.map(({ shard }) => shard));
    return Array.from({ length: of }, (_, at) => at + 1)
      .filter((shard) => !have.has(shard))
      .map((shard) => `${browser} ${shard} of ${of} wrote no report`);
  });
}

/** The shards, each `{ name, report }`, whose stories did not pass and that left no
 *  `drew-<name>` among `markers`: a report that does not say it passed counts as failed. */
export function undrawn(shards, markers) {
  const drew = new Set(markers);
  return shards
    .filter(({ report }) => report?.failed !== false)
    .filter(({ name }) => !drew.has(`drew-${name}`))
    .map(({ name }) => name);
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

/**
 * What failed in one browser's report, each as `{ blocks, line, named, why, listed? }`: its
 * `::error` annotation (`line`), whether it stops the run drawing baselines (`blocks`), what
 * names it in the first annotation, and why, which the summary says after it (a story's own
 * line there is `listed`). One per failed story: the story's name and that browser, its cell,
 * its file, its gate, why, and the axe rules it broke; one for each error outside any story;
 * for a run that failed naming nothing, one that says so; and for a report the run never wrote
 * (`report` undefined), one that says that. The report does not say which browser ran it, so the
 * job runs each browser on its own and says which report is whose.
 */
export function failedIn(report, browser, root) {
  if (!report) {
    const why = "the run wrote no report";
    return [
      {
        blocks: true,
        line: error(`stories in ${browser}`, `${why}; the job's log says why`),
        named: `stories in ${browser}: ${why}`,
        why: "the job's log says why",
      },
    ];
  }
  const failed = report.failures.map((failure) => {
    const { file, story, cell, errors } = failure;
    const blocks = blocksBaselines(failure);
    // The error that blocks the baselines is the one worth naming, when there is one.
    const [first = { message: "" }] = [
      ...errors.filter((one) => gateOf(one) !== "screenshot"),
      ...errors,
    ];
    const rules = [
      ...new Set(
        errors.flatMap((one) =>
          [...(one.message ?? "").matchAll(RULE)].map((found) => found[1]),
        ),
      ),
    ];
    const broke = rules.length > 0 ? ` [axe: ${rules.join(", ")}]` : "";
    const title = `${story} in ${browser}${cell ? `, ${cell}` : ""}`;
    const gates = [...new Set(errors.map(gateOf).filter((gate) => gate !== "screenshot"))];
    const why = `${gateOf(first)}: ${whyOf(first)}${broke}`;
    return {
      blocks,
      line: error(
        title,
        `${file}: ${why}`,
        root === undefined ? undefined : placeOf(first, root, file),
      ),
      named: `${title} (${file})${blocks ? `: ${gates.join(", ") || "renders"}` : ""}`,
      why,
      // The summary's line: its gate leads its why already.
      listed: `${title} (${file}): ${why}`,
      // The headline names the story once, with the browsers and the cells it failed in.
      once: `${story} (${file}): ${gates.join(", ") || "renders"}`,
      browser,
      cell,
    };
  });
  const outside = report.errors.map((one) => {
    const why = whyOf(one);
    return {
      blocks: true,
      line: error(
        `stories in ${browser}`,
        `${one.file ? `${one.file}: ` : ""}an error outside any story: ${why}`,
      ),
      named: `${one.file ? `${one.file} in ` : "stories in "}${browser}: an error outside any story`,
      why,
    };
  });
  const all = [...failed, ...outside];
  if (all.length === 0 && report.failed) {
    const why = "the run failed but named no story";
    return [
      {
        blocks: true,
        line: error(`stories in ${browser}`, `${why}; the job's log says why`),
        named: `stories in ${browser}: ${why}`,
        why: "the job's log says why",
      },
    ];
  }
  return all;
}

/** One `::error` line per failed story in one browser's report (`failedIn`). */
export function annotations(report, browser, root) {
  return failedIn(report, browser, root).map(({ line }) => line);
}

/** `count` stories, said with `one` or `many` after it. */
const stories = (count, one, many) =>
  `${count} ${count === 1 ? `story ${one}` : `stories ${many}`}`;

/** Text kept from breaking a Markdown list item or its code span. */
const plain = (text) => String(text).replace(/[\r\n]+/g, " ").replace(/`/g, "'");

/**
 * **What failed across every report of a run, blocking first** (#4050): each `{ browser, report }`
 * (a report the run never wrote is `undefined`), and what is `missing` of the shards' reports
 * (`shardProblems`), each said in a line. GitHub keeps the first 10 error annotations of
 * a step, so the stories that stop the baselines being drawn (failing on more than their
 * picture, and any error outside a story, run that failed naming nothing or report never
 * written) are `annotations` before any picture's. The `headline` names every one of them in one
 * annotation, each story once with the browsers and cells it failed in; the job prints it in a
 * step of its own after every other, as GitHub shows a job's newest annotation first. The
 * `summary`, Markdown for the step's summary, which GitHub never caps, names the same first and
 * then every picture that failed alone.
 */
export function named(runs, root, missing = []) {
  const all = [
    ...runs.flatMap(({ browser, report }) => failedIn(report, browser, root)),
    ...missing.map((said) => ({
      blocks: true,
      line: error("stories", `${said}; its log says why`),
      named: said,
      why: "its log says why",
    })),
  ];
  const blocking = all.filter(({ blocks }) => blocks);
  const alone = all.filter(({ blocks }) => !blocks);
  const lines = [...blocking, ...alone].map(({ line }) => line);
  // Each blocking story once, with where it failed: browser, then its cells.
  const grouped = new Map();
  for (const { once, named: name, browser, cell } of blocking) {
    const key = once ?? name;
    if (!grouped.has(key)) grouped.set(key, new Map());
    if (once) {
      const browsers = grouped.get(key);
      if (!browsers.has(browser)) browsers.set(browser, new Set());
      if (cell) browsers.get(browser).add(cell);
    }
  }
  const heads = [...grouped].map(([key, browsers]) =>
    browsers.size === 0
      ? key
      : `${key}, in ${[...browsers]
          .map(([browser, cells]) =>
            cells.size === 0 ? browser : `${browser} (${[...cells].sort().join(", ")})`,
          )
          .join(" and ")}`,
  );
  const said = stories(
    heads.length,
    "failed on more than its picture",
    "failed on more than their picture",
  );
  const headline =
    blocking.length > 0 ? error(`No baselines drawn: ${said}`, heads.join("\n")) : undefined;
  const sections = [];
  if (blocking.length > 0)
    sections.push(
      `## ${said}`,
      "",
      "No baselines are drawn until these pass; then the next run draws them.",
      "",
      ...blocking.map(({ named: name, why, listed }) => `- ${plain(listed ?? `${name}: ${why}`)}`),
      "",
    );
  if (alone.length > 0)
    sections.push(
      `## ${stories(alone.length, "failed on its picture alone", "failed on their picture alone")}`,
      "",
      ...alone.map(({ named: name }) => `- ${plain(name)}`),
      "",
    );
  return { headline, annotations: lines, summary: sections.join("\n") };
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

if (
  process.argv[1] === fileURLToPath(import.meta.url) &&
  process.argv[2] === "pictures-alone"
) {
  // Exits 0 when a picture is all that failed; a report the run did not write is not that.
  let reports;
  try {
    reports = process.argv
      .slice(3)
      .map((path) => JSON.parse(readFileSync(path, "utf8")));
  } catch (error) {
    console.error(`a stories report could not be read: ${error.message}`);
    process.exit(1);
  }
  if (reports.length === 0 || !picturesAlone(reports)) process.exit(1);
  const files = new Set(
    reports.flatMap((report) => report.failures.map(({ file }) => file)),
  );
  for (const file of files) console.log(file);
} else if (
  process.argv[1] === fileURLToPath(import.meta.url) &&
  process.argv[2] === "pictures-to-draw"
) {
  let pairs;
  try {
    pairs = picturesToDraw(JSON.parse(readFileSync(process.argv[3], "utf8")));
  } catch (error) {
    console.error(`a stories report could not be read: ${error.message}`);
    process.exit(1);
  }
  if (!pairs || pairs.length === 0) process.exit(1);
  for (const { file, cell } of pairs) console.log(`${file}\t${cell}`);
} else if (
  process.argv[1] === fileURLToPath(import.meta.url) &&
  process.argv[2] === "shards"
) {
  const [reports, drawn] = process.argv.slice(3);
  const names = existsSync(reports) ? readdirSync(reports) : [];
  const found = shardProblems(names);
  if (drawn !== undefined && found.length === 0) {
    const shards = names.flatMap((fileName) => {
      const shard = shardOf(fileName);
      if (!shard) return [];
      let report;
      try {
        report = JSON.parse(readFileSync(join(reports, fileName), "utf8"));
      } catch {
        report = undefined;
      }
      return [{ name: shard.name, report }];
    });
    const markers = existsSync(drawn) ? readdirSync(drawn) : [];
    for (const name of undrawn(shards, markers))
      found.push(`${name} failed and drew no pictures`);
  }
  for (const line of found) console.log(line);
  if (found.length > 0) process.exit(1);
} else if (
  process.argv[1] === fileURLToPath(import.meta.url) &&
  process.argv[2] === "headline"
) {
  // The check's: every shard's report in one directory, by its shard's name (`shardOf`).
  const reports = process.argv[3];
  const names = existsSync(reports) ? readdirSync(reports).sort() : [];
  const runs = names.flatMap((fileName) => {
    const shard = shardOf(fileName);
    if (!shard) return [];
    let report;
    try {
      report = JSON.parse(readFileSync(join(reports, fileName), "utf8"));
    } catch {
      report = undefined;
    }
    return [{ browser: shard.browser, report }];
  });
  const { headline } = named(runs, process.env.GITHUB_WORKSPACE || process.cwd(), shardProblems(names));
  if (headline) console.log(headline);
} else if (process.argv[1] === fileURLToPath(import.meta.url)) {
  // `<browser> <report>`, once for each report a browser's shards wrote: the annotations of
  // every story that failed, blocking first, and the step's summary.
  const given = process.argv.slice(2);
  // The repository, which an annotation's file is relative to.
  const root = process.env.GITHUB_WORKSPACE || process.cwd();
  const runs = [];
  for (let at = 0; at + 1 < given.length; at += 2) {
    let report;
    try {
      report = JSON.parse(readFileSync(given[at + 1], "utf8"));
    } catch {
      report = undefined;
    }
    runs.push({ browser: given[at], report });
  }
  const { annotations: lines, summary } = named(runs, root);
  for (const line of lines) console.log(line);
  if (summary && process.env.GITHUB_STEP_SUMMARY)
    appendFileSync(process.env.GITHUB_STEP_SUMMARY, `${summary}\n`);
}
