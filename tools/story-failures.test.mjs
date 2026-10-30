// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The stories job's annotation for each story that failed (#2901, #2944), and the reporter that
// keeps what it is made from. Run with `node --test tools/`.
import assert from "node:assert/strict";
import { mkdtempSync, readdirSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

import StoryFailures, {
  annotations,
  cellOf,
  gateOf,
  picturesAlone,
  picturesToDraw,
  placeOf,
  REPORT,
  reportPath,
  shardOf,
  shardProblems,
  undrawn,
} from "./story-failures.mjs";

const DEPS = "http://localhost:1/node_modules/.cache/storybook/sb-vitest/deps";
const STORY = "http://localhost:1/__w/purlis/purlis/app/src/Probe.stories.tsx";

// Each gate's failure as the two engines report it. WebKit's stack is frames alone; Chromium's
// begins with the message. These are the shapes a deliberate failure of each gate gave in the
// pinned image (#2944).
const FAILED = {
  renders: {
    message: "the probe would not draw",
    chromium: `Error: the probe would not draw\n    at Probe (${STORY}?import:9:34)\n    at renderWithHooks (${DEPS}/react-dom_client.js:15252:19)`,
    webkit: `Probe@${STORY}:9:43\nreact_stack_bottom_frame@${DEPS}/react-dom_client.js:24994:21\nrenderWithHooks@${DEPS}/react-dom_client.js:15252:37`,
  },
  keyboard: {
    message:
      "expect(element).not.toHaveFocus()\n\nReceived element is focused:\n  <button>",
    chromium: `Error: expect(element).not.toHaveFocus()\n    at Proxy.expectWrapper (${DEPS}/test.js:26013:49)\n    at play (${STORY}?import:49:13)\n    at async runStory (${DEPS}/csf.js:8956:6)`,
    webkit: `expectWrapper@${DEPS}/test.js:26013:68\n@[native code]\n@${STORY}:49:24\nasync runStory@${DEPS}/csf.js:8956:24`,
  },
  accessibility: {
    message:
      "\u001b[2mexpect(\u001b[22mreceived).toHaveNoViolations(expected)\n\nExpected the HTML found at $('button') to have no violations:\n\n" +
      'Received:\n\n\u001b[31m"Buttons must have discernible text (button-name)"\u001b[39m',
    chromium: `Error: expect(received).toHaveNoViolations(expected)\n    at Proxy.expectWrapper (${DEPS}/test.js:26013:49)\n    at afterEach (${DEPS}/addon-a11y.js:1:2)`,
    webkit: `expectWrapper@${DEPS}/test.js:26013:68\n@http://localhost:1/node_modules/@storybook/addon-a11y/dist/chunk.js:165:44\nasync runStory@${DEPS}/csf.js:8958:126`,
  },
  screenshot: {
    message:
      "expect(\u001b[22m\u001b[31melement\u001b[39m).toMatchScreenshot()\n\nNo existing reference screenshot found.\n\nReference screenshot:\n  /a.png",
    chromium: `Error: expect(element).toMatchScreenshot()\n    at Proxy.__VITEST_POLL_CHAIN__ (${DEPS}/index.js:6450:47)\n    at http://localhost:1/__w/purlis/purlis/app/.storybook/screenshots.setup.ts?import:31:51`,
    webkit: `__VITEST_POLL_CHAIN__@${DEPS}/index.js:6450:56\n@http://localhost:1/__w/purlis/purlis/app/.storybook/screenshots.setup.ts:31:68`,
  },
};

// Storybook's Vitest addon puts a link to the story before the message of each one that failed.
const LINKED = (message) =>
  "\n\u001b[34mClick to debug the error directly in Storybook: http://localhost:6006/?path=/story/probe--a\u001b[39m\n\n" +
  message;

const reportOf = (story, error) => ({
  failures: [{ file: "src/Probe.stories.tsx", story, errors: [error] }],
  errors: [],
});

const WHY = {
  renders: "the probe would not draw",
  keyboard: "expect(element).not.toHaveFocus(): Received element is focused",
  accessibility:
    "expect(received).toHaveNoViolations(expected): Expected the HTML found at $('button') to have no violations [axe: button-name]",
  screenshot:
    "expect(element).toMatchScreenshot(): No existing reference screenshot found.",
};

for (const [gate, failed] of Object.entries(FAILED)) {
  test(`a story failing its ${gate} gate says the same in WebKit as in Chromium`, () => {
    for (const browser of ["chromium", "webkit"]) {
      const message =
        gate === "screenshot" ? failed.message : LINKED(failed.message);
      const error = { message, stack: failed[browser] };
      assert.equal(gateOf(error), gate, `${gate} in ${browser}`);
      assert.deepEqual(annotations(reportOf("Probe Story", error), browser), [
        `::error title=Probe Story in ${browser}::src/Probe.stories.tsx: ${gate}: ${WHY[gate]}`,
      ]);
    }
  });
}

test("a failure in a play function's waitFor, whose stack is the story's code, is the keyboard's", () => {
  const error = {
    message: "Timed out in waitFor.",
    stack: `waitFor@${DEPS}/test.js:1:2\nplay@${STORY}:30:9`,
  };
  assert.equal(gateOf(error), "keyboard");
});

test("a failure with no message says so", () => {
  const [one] = annotations(
    reportOf("A", { message: "", stack: "@[native code]" }),
    "webkit",
  );
  assert.match(one, /no message/);
});

test("a report with nothing failed says nothing", () => {
  assert.deepEqual(annotations({ failures: [], errors: [] }, "chromium"), []);
});

test("an error outside any story is named for its browser", () => {
  assert.deepEqual(
    annotations(
      {
        failures: [],
        errors: [{ message: "Failed to load the setup file", stack: "" }],
      },
      "webkit",
    ),
    [
      "::error title=stories in webkit::an error outside any story: Failed to load the setup file",
    ],
  );
});

test("a line that would end the annotation early is kept on one line", () => {
  const [one] = annotations(
    reportOf("A::B", { message: "x".repeat(500), stack: "" }),
    "chromium",
  );
  assert.equal(one.includes("\n"), false);
  assert.ok(one.length < 400);
  assert.match(one, /title=A%3A%3AB in chromium::/);
});

test("annotation text is escaped, so each failure stays one annotation on one line", () => {
  const message = "boom\r::warning::x\r\nmore 100%";
  const [story] = annotations(
    {
      failures: [
        { file: "src/A::B.stories.tsx", story: "A", errors: [{ message }] },
      ],
      errors: [],
    },
    "webkit",
  );
  assert.equal(
    story,
    "::error title=A in webkit::src/A::B.stories.tsx: renders: boom",
  );
  const [outside] = annotations(
    { failures: [], errors: [{ message: "50%0A done\r::add-mask::y" }] },
    "webkit",
  );
  assert.equal(
    outside,
    "::error title=stories in webkit::an error outside any story: 50%250A done",
  );
  const [cut] = annotations(
    reportOf("A", { message: `${"x".repeat(299)}%\n`, stack: "" }),
    "chromium",
  );
  assert.match(cut, /x%25$/);
  for (const line of [story, outside, cut]) assert.doesNotMatch(line, /[\r\n]/);
});

test("the reason is cut by characters, never inside one", () => {
  const [one] = annotations(
    reportOf("A", { message: `${"x".repeat(299)}\u{1F600}y`, stack: "" }),
    "chromium",
  );
  assert.ok(one.endsWith("x\u{1F600}"));
});

test("a run that failed naming no story says so, for the browser it failed in", () => {
  assert.deepEqual(
    annotations({ failed: true, failures: [], errors: [] }, "webkit"),
    [
      "::error title=stories in webkit::the run failed but named no story; the job's log says why",
    ],
  );
  assert.deepEqual(
    annotations({ failed: false, failures: [], errors: [] }, "chromium"),
    [],
  );
});

test("a loader or beforeEach that throws is the renders gate's", () => {
  for (const stack of [
    `Error: no data\n    at loaders (${STORY}?import:12:9)\n    at applyLoaders (${DEPS}/csf.js:751:9)`,
    `@${STORY}:12:9\napplyLoaders@${DEPS}/csf.js:751:9\nasync runStory@${DEPS}/csf.js:1210:24`,
    `@${STORY}:12:9\napplyBeforeEach@${DEPS}/csf.js:764:9`,
  ])
    assert.equal(gateOf({ message: "no data", stack }), "renders");
  assert.equal(
    gateOf({ message: "x", stack: `@${DEPS}/replay.js:1:2` }),
    "renders",
  );
});

test("the report goes only to the absolute path the variable names", () => {
  assert.equal(reportPath({}), null);
  assert.equal(reportPath({ [REPORT]: "" }), null);
  assert.equal(reportPath({ [REPORT]: "app/src/x.json" }), null);
  assert.equal(reportPath({ [REPORT]: "/tmp/r.json" }), "/tmp/r.json");
});

/** Runs the reporter with `PURLIS_STORY_REPORT` set to `to` (unset for `undefined`). */
function reporting(to, run) {
  const before = process.env[REPORT];
  if (to === undefined) delete process.env[REPORT];
  else process.env[REPORT] = to;
  try {
    run(new StoryFailures());
  } finally {
    if (before === undefined) delete process.env[REPORT];
    else process.env[REPORT] = before;
  }
}

/** A story file as Vitest's reporter API hands it over. */
function moduleOf({ tests = [], errors = [], suites = [] }) {
  return {
    relativeModuleId: "src/Probe.stories.tsx",
    errors: () => errors,
    children: {
      *allTests(state) {
        for (const one of tests)
          if (state === undefined || (one.state ?? "failed") === state)
            yield { meta: () => ({}), ...one };
      },
      *allSuites() {
        yield* suites.map((suiteErrors) => ({ errors: () => suiteErrors }));
      },
    },
  };
}

test("the reporter keeps each failed story's message apart from its stack", () => {
  const to = join(
    mkdtempSync(join(tmpdir(), "story-failures-")),
    "report.json",
  );
  const failedTest = {
    fullName: "Probe > Steps",
    result: () => ({
      errors: [
        { message: FAILED.keyboard.message, stack: FAILED.keyboard.webkit },
      ],
    }),
  };
  reporting(to, (reporter) =>
    reporter.onTestRunEnd(
      [moduleOf({ tests: [failedTest] })],
      [{ message: "outside", stack: "s" }],
      "failed",
    ),
  );
  const report = JSON.parse(readFileSync(to, "utf8"));
  assert.deepEqual(report, {
    failed: true,
    failures: [
      {
        file: "src/Probe.stories.tsx",
        story: "Probe Steps",
        errors: [
          {
            message: FAILED.keyboard.message,
            stack: FAILED.keyboard.webkit,
            frames: [],
          },
        ],
      },
    ],
    errors: [{ message: "outside", stack: "s", frames: [] }],
    unpictured: [],
  });
  assert.deepEqual(annotations(report, "webkit"), [
    `::error title=Probe Steps in webkit::src/Probe.stories.tsx: keyboard: ${WHY.keyboard}`,
    "::error title=stories in webkit::an error outside any story: outside",
  ]);
});

test("the reporter keeps a story file's and a suite's own errors, with the file", () => {
  const to = join(
    mkdtempSync(join(tmpdir(), "story-failures-")),
    "report.json",
  );
  reporting(to, (reporter) =>
    reporter.onTestRunEnd(
      [
        moduleOf({
          errors: [{ message: "Failed to import ./components", stack: "" }],
          suites: [[{ message: "beforeAll threw", stack: "" }]],
        }),
      ],
      [],
      "failed",
    ),
  );
  const report = JSON.parse(readFileSync(to, "utf8"));
  assert.deepEqual(annotations(report, "webkit"), [
    "::error title=stories in webkit::src/Probe.stories.tsx: an error outside any story: Failed to import ./components",
    "::error title=stories in webkit::src/Probe.stories.tsx: an error outside any story: beforeAll threw",
  ]);
});

test("the reporter writes nothing when the variable is unset or not absolute", () => {
  const dir = mkdtempSync(join(tmpdir(), "story-failures-"));
  const before = process.cwd();
  process.chdir(dir);
  try {
    for (const to of [undefined, "report.json"])
      reporting(to, (reporter) =>
        reporter.onTestRunEnd([moduleOf({})], [], "failed"),
      );
  } finally {
    process.chdir(before);
  }
  assert.deepEqual(readdirSync(dir), []);
});

test("a failed story is named with the matrix cell it failed in", () => {
  assert.equal(cellOf("stories purlis_dark (webkit)"), "purlis_dark");
  assert.equal(
    cellOf("stories purlis-classic_light_compact (chromium)"),
    "purlis-classic_light_compact",
  );
  assert.equal(cellOf("unit"), undefined);
  const to = join(
    mkdtempSync(join(tmpdir(), "story-failures-")),
    "report.json",
  );
  const failedTest = {
    fullName: "Probe > Steps",
    result: () => ({
      errors: [{ message: "the probe would not draw", stack: "" }],
    }),
  };
  reporting(to, (reporter) =>
    reporter.onTestRunEnd(
      [
        {
          ...moduleOf({ tests: [failedTest] }),
          project: { name: "stories purlis_light (webkit)" },
        },
      ],
      [],
      "failed",
    ),
  );
  const report = JSON.parse(readFileSync(to, "utf8"));
  assert.deepEqual(annotations(report, "webkit"), [
    "::error title=Probe Steps in webkit%2C purlis_light::src/Probe.stories.tsx: renders: the probe would not draw",
  ]);
});

test("the reporter names each story that takes no picture, once", () => {
  const to = join(
    mkdtempSync(join(tmpdir(), "story-failures-")),
    "report.json",
  );
  const off = (state) => ({
    state,
    fullName: "Probe > Moving",
    meta: () => ({ screenshot: { story: "probe--moving", taken: false } }),
    result: () => ({ errors: [] }),
  });
  const pictured = {
    state: "passed",
    fullName: "Probe > Still",
    meta: () => ({ screenshot: { story: "probe--still", taken: "frame" } }),
    result: () => ({ errors: [] }),
  };
  reporting(to, (reporter) =>
    reporter.onTestRunEnd(
      [
        moduleOf({ tests: [off("passed"), pictured] }),
        moduleOf({ tests: [off("passed")] }),
        moduleOf({
          tests: [
            {
              ...off("failed"),
              meta: () => ({
                screenshot: { story: "probe--other", taken: false },
              }),
            },
          ],
        }),
      ],
      [],
      "failed",
    ),
  );
  assert.deepEqual(JSON.parse(readFileSync(to, "utf8")).unpictured, [
    { file: "src/Probe.stories.tsx", id: "probe--moving" },
    { file: "src/Probe.stories.tsx", id: "probe--other" },
  ]);
});

const ROOT = "/__w/purlis/purlis";

/** The frames Vitest maps a keyboard failure's stack back to, in each engine's shape. */
const MAPPED = [
  {
    file: `${ROOT}/app/node_modules/@vitest/expect/dist/index.js`,
    line: 1,
    column: 1,
  },
  { file: `${ROOT}/app/src/stories.testkit.ts`, line: 40, column: 3 },
  { file: `${ROOT}/app/src/Probe.stories.tsx`, line: 47, column: 11 },
  { file: `${ROOT}/app/src/Other.stories.tsx`, line: 3, column: 1 },
];

test("a failed story's annotation names its story file and the line it failed at", () => {
  const [one] = annotations(
    reportOf("Probe Steps", {
      message: FAILED.keyboard.message,
      stack: FAILED.keyboard.webkit,
      frames: MAPPED,
    }),
    "webkit",
    ROOT,
  );
  assert.equal(
    one,
    `::error file=app/src/Probe.stories.tsx,line=47,title=Probe Steps in webkit::src/Probe.stories.tsx: keyboard: ${WHY.keyboard}`,
  );
});

test("a frame is placed whether Vitest gives it as a path, a served URL or a file URL", () => {
  for (const file of [
    `${ROOT}/app/src/Probe.stories.tsx`,
    `http://localhost:63315${ROOT}/app/src/Probe.stories.tsx?import`,
    `http://localhost:63315/@fs${ROOT}/app/src/Probe.stories.tsx`,
    `file://${ROOT}/app/src/Probe.stories.tsx`,
  ])
    assert.deepEqual(
      placeOf({ frames: [{ file, line: 9.0, column: 2 }] }, ROOT),
      { file: "app/src/Probe.stories.tsx", line: 9 },
      file,
    );
});

test("a failure with no frame in a story file inside the repository keeps its annotation", () => {
  for (const frames of [
    undefined,
    [],
    [
      {
        file: `${ROOT}/app/.storybook/screenshots.setup.ts`,
        line: 31,
        column: 5,
      },
    ],
    [{ file: "/elsewhere/src/Probe.stories.tsx", line: 4, column: 1 }],
    [{ file: `${ROOT}/app/src/Probe.stories.tsx`, line: 0, column: 0 }],
  ]) {
    assert.equal(placeOf({ frames }, ROOT), undefined);
    const [one] = annotations(
      reportOf("A", { message: "the probe would not draw", stack: "", frames }),
      "chromium",
      ROOT,
    );
    assert.match(one, /^::error title=A in chromium::/);
  }
});

test("the reporter keeps the frames Vitest mapped back to their sources", () => {
  const to = join(
    mkdtempSync(join(tmpdir(), "story-failures-")),
    "report.json",
  );
  const failedTest = {
    fullName: "Probe > Steps",
    result: () => ({
      errors: [
        {
          message: "m",
          stack: "s",
          stacks: [
            { method: "play", file: MAPPED[2].file, line: 47, column: 11 },
          ],
        },
      ],
    }),
  };
  reporting(to, (reporter) =>
    reporter.onTestRunEnd([moduleOf({ tests: [failedTest] })], [], "failed"),
  );
  const report = JSON.parse(readFileSync(to, "utf8"));
  assert.deepEqual(report.failures[0].errors[0].frames, [MAPPED[2]]);
});

test("the failed story's own file is placed before another story file's frame", () => {
  const frames = [
    { file: `${ROOT}/app/src/Other.stories.tsx`, line: 3, column: 1 },
    { file: `${ROOT}/app/src/Probe.stories.tsx`, line: 47, column: 11 },
  ];
  assert.deepEqual(placeOf({ frames }, ROOT, "src/Probe.stories.tsx"), {
    file: "app/src/Probe.stories.tsx",
    line: 47,
  });
  assert.deepEqual(placeOf({ frames }, ROOT, "src/Gone.stories.tsx"), {
    file: "app/src/Other.stories.tsx",
    line: 3,
  });
  assert.deepEqual(placeOf({ frames }, ROOT), {
    file: "app/src/Other.stories.tsx",
    line: 3,
  });
});

// When the stories job draws baselines for its pull request to take (`stories.yml`): only when
// a picture is all that failed, in every browser's report.
const failedAt = (gate, browser = "chromium") => ({
  message: FAILED[gate].message,
  stack: FAILED[gate][browser],
});

test("a run whose stories failed on their pictures alone is drawn for its pull request", () => {
  assert.equal(
    picturesAlone([
      reportOf("Probe A", failedAt("screenshot")),
      { failed: true, ...reportOf("Probe B", failedAt("screenshot", "webkit")) },
    ]),
    true,
  );
  // One browser's run passed: the other's pictures alone still count.
  assert.equal(
    picturesAlone([
      reportOf("Probe A", failedAt("screenshot")),
      { failed: false, failures: [], errors: [] },
    ]),
    true,
  );
});

test("a run with any failure but a picture's draws nothing", () => {
  for (const gate of ["renders", "keyboard", "accessibility"]) {
    assert.equal(
      picturesAlone([reportOf("Probe A", failedAt("screenshot")), reportOf("Probe B", failedAt(gate))]),
      false,
      gate,
    );
    // A story that failed its picture and another gate is not a picture alone either.
    assert.equal(
      picturesAlone([
        {
          failures: [
            {
              file: "src/Probe.stories.tsx",
              story: "Probe A",
              errors: [failedAt("screenshot"), failedAt(gate)],
            },
          ],
          errors: [],
        },
      ]),
      false,
      `${gate} beside a picture`,
    );
  }
});

test("an error outside a story, a run that failed naming nothing, or no failure draws nothing", () => {
  assert.equal(
    picturesAlone([
      {
        ...reportOf("Probe A", failedAt("screenshot")),
        errors: [{ message: "the file would not load", stack: "" }],
      },
    ]),
    false,
  );
  assert.equal(
    picturesAlone([
      reportOf("Probe A", failedAt("screenshot")),
      { failed: true, failures: [], errors: [] },
    ]),
    false,
  );
  assert.equal(picturesAlone([{ failed: false, failures: [], errors: [] }]), false);
  assert.equal(picturesAlone([]), false);
  assert.equal(
    picturesAlone([
      { failures: [{ file: "src/Probe.stories.tsx", story: "Probe A", errors: [] }], errors: [] },
    ]),
    false,
  );
});

// The stories run in shards (#4013): a shard draws again only the pictures of its own story
// files in its own cells, and the check counts every shard's report and drawing.
const pictureFailure = (file, cell) => ({
  file,
  story: "Probe A",
  cell,
  errors: [failedAt("screenshot")],
});

test("a shard draws again each story file and cell whose picture failed, once each", () => {
  assert.deepEqual(
    picturesToDraw({
      failed: true,
      errors: [],
      failures: [
        pictureFailure("src/A.stories.tsx", "purlis_dark"),
        pictureFailure("src/A.stories.tsx", "purlis_dark"),
        pictureFailure("src/A.stories.tsx", "purlis-classic_light"),
        pictureFailure("src/B.stories.tsx", "purlis_dark"),
      ],
    }),
    [
      { file: "src/A.stories.tsx", cell: "purlis_dark" },
      { file: "src/A.stories.tsx", cell: "purlis-classic_light" },
      { file: "src/B.stories.tsx", cell: "purlis_dark" },
    ],
  );
});

test("a shard draws nothing when more than a picture failed, or a failure names no cell", () => {
  assert.equal(
    picturesToDraw(reportOf("Probe A", failedAt("keyboard"))),
    undefined,
  );
  assert.equal(
    picturesToDraw({
      failed: true,
      errors: [],
      failures: [pictureFailure("src/A.stories.tsx", undefined)],
    }),
    undefined,
  );
});

test("a shard's name is its browser, its number and how many there are", () => {
  assert.deepEqual(shardOf("report-webkit-2-of-3.json"), {
    browser: "webkit",
    shard: 2,
    of: 3,
    name: "webkit-2-of-3",
  });
  assert.equal(shardOf("index.json"), undefined);
  assert.equal(shardOf("report-webkit-4-of-3.json"), undefined);
});

test("every shard of every browser reported, or the check names the one missing", () => {
  const all = [
    "report-chromium-1-of-2.json",
    "report-chromium-2-of-2.json",
    "report-webkit-1-of-2.json",
    "report-webkit-2-of-2.json",
  ];
  assert.deepEqual(shardProblems(all), []);
  assert.deepEqual(shardProblems(all.slice(1)), [
    "chromium 1 of 2 wrote no report",
  ]);
  // A browser none of whose shards reported.
  assert.deepEqual(shardProblems(all.slice(0, 2)), ["webkit: no shard wrote a report"]);
  // A matrix that runs fewer shards than `--shard` splits the files into.
  assert.deepEqual(
    shardProblems([
      "report-chromium-1-of-3.json",
      "report-chromium-2-of-3.json",
      "report-webkit-1-of-2.json",
      "report-webkit-2-of-2.json",
    ]),
    ["chromium 3 of 3 wrote no report"],
  );
  // Shards that disagree on how many there are.
  assert.deepEqual(
    shardProblems([
      "report-chromium-1-of-2.json",
      "report-chromium-2-of-3.json",
      "report-webkit-1-of-1.json",
    ]),
    ["chromium: its shards disagree on how many there are (2, 3)"],
  );
});

test("each shard that failed drew its pictures, or the check names it", () => {
  const passed = { failed: false, failures: [], errors: [] };
  const failed = { failed: true, failures: [], errors: [] };
  assert.deepEqual(
    undrawn(
      [
        { name: "chromium-1-of-2", report: passed },
        { name: "chromium-2-of-2", report: failed },
        { name: "webkit-1-of-2", report: failed },
      ],
      ["drew-webkit-1-of-2"],
    ),
    ["chromium-2-of-2"],
  );
  // A report whose `failed` is not a boolean is not read as one that passed.
  assert.deepEqual(undrawn([{ name: "webkit-2-of-2", report: {} }], []), [
    "webkit-2-of-2",
  ]);
});
