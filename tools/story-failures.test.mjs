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

test("each failed story is one error, named, with the first line of why", () => {
  assert.deepEqual(annotations(REPORT), [
    "::error title=Gates Unnamed::src/settings/Choice.stories.tsx: Expected the HTML found at $('button') to have no violations:",
  ]);
});

test("a report with nothing failed says nothing", () => {
  assert.deepEqual(annotations({ testResults: [] }), []);
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
  const [one] = annotations(report);
  assert.equal(one.includes("\n"), false);
  assert.ok(one.length < 400);
  assert.match(one, /title=A%3A%3AB::/);
});
