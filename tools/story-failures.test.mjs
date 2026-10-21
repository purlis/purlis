// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The stories job's annotation for each story that failed (#2901). Run with `node --test tools/`.
import assert from "node:assert/strict";
import { test } from "node:test";

import { annotations } from "./story-failures.mjs";

const REPORT = {
  testResults: [
    {
      name: "/w/purlis/app/src/settings/Choice.stories.tsx",
      assertionResults: [
        { status: "passed", fullName: "Choice Radios", failureMessages: [] },
        {
          status: "failed",
          fullName: "Gates Unnamed",
          failureMessages: [
            "\nExpected the HTML found at $('button') to have no violations:\nmore",
          ],
        },
      ],
    },
  ],
};

test("each failed story is one error, named with its browser, with the first line of why", () => {
  assert.deepEqual(annotations(REPORT, "webkit"), [
    "::error title=Gates Unnamed in webkit::src/settings/Choice.stories.tsx: Expected the HTML found at $('button') to have no violations:",
  ]);
});

test("colour codes and stack frames are left out, and the axe rules broken are named", () => {
  const report = {
    testResults: [
      {
        name: "/w/app/src/Gates.stories.tsx",
        assertionResults: [
          {
            status: "failed",
            fullName: "Small Targets",
            failureMessages: [
              "expectWrapper@http://localhost:1/deps/test.js:1:2\nError: \u001b[2mexpect(\u001b[22mreceived).toHaveNoViolations()\n" +
                'Received:\n  "All touch targets must be 24px large, or leave sufficient space (target-size)"\n' +
                'Received:\n  "All touch targets must be 24px large, or leave sufficient space (target-size)"',
            ],
          },
        ],
      },
    ],
  };
  assert.deepEqual(annotations(report, "chromium"), [
    "::error title=Small Targets in chromium::src/Gates.stories.tsx: Error: expect(received).toHaveNoViolations() [axe: target-size]",
  ]);
});

test("a failure that is only a stack trace says so", () => {
  const report = {
    testResults: [
      {
        name: "/w/app/src/A.stories.tsx",
        assertionResults: [
          {
            status: "failed",
            fullName: "A",
            failureMessages: ["@[native code]\nf@http://x/y.js:1:2"],
          },
        ],
      },
    ],
  };
  assert.match(annotations(report, "webkit")[0], /only a stack trace/);
});

test("a report with nothing failed says nothing", () => {
  assert.deepEqual(annotations({ testResults: [] }, "chromium"), []);
});

test("a line that would end the annotation early is kept on one line", () => {
  const report = {
    testResults: [
      {
        name: "/a/app/src/A.stories.tsx",
        assertionResults: [
          {
            status: "failed",
            fullName: "A::B",
            failureMessages: ["x".repeat(500)],
          },
        ],
      },
    ],
  };
  const [one] = annotations(report, "chromium");
  assert.equal(one.includes("\n"), false);
  assert.ok(one.length < 400);
  assert.match(one, /title=A%3A%3AB in chromium::/);
});
