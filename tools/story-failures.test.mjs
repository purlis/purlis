// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The stories job's annotation for each story that failed (#2901, #2944), and the reporter that
// keeps what it is made from. Run with `node --test tools/`.
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

import StoryFailures, {
  annotations,
  gateOf,
  REPORT,
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
      const error = { message: failed.message, stack: failed[browser] };
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
  const module = {
    relativeModuleId: "src/Probe.stories.tsx",
    children: {
      *allTests(state) {
        assert.equal(state, "failed");
        yield failedTest;
      },
    },
  };
  const before = process.env[REPORT];
  process.env[REPORT] = to;
  try {
    new StoryFailures().onTestRunEnd(
      [module],
      [{ message: "outside", stack: "s" }],
    );
  } finally {
    if (before === undefined) delete process.env[REPORT];
    else process.env[REPORT] = before;
  }
  const report = JSON.parse(readFileSync(to, "utf8"));
  assert.deepEqual(report, {
    failures: [
      {
        file: "src/Probe.stories.tsx",
        story: "Probe Steps",
        errors: [
          { message: FAILED.keyboard.message, stack: FAILED.keyboard.webkit },
        ],
      },
    ],
    errors: [{ message: "outside", stack: "s" }],
  });
  assert.deepEqual(annotations(report, "webkit"), [
    `::error title=Probe Steps in webkit::src/Probe.stories.tsx: keyboard: ${WHY.keyboard}`,
    "::error title=stories in webkit::an error outside any story: outside",
  ]);
});
