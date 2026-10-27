// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The activity family's lint** (`lint.js` beside this file, #2917): what it refuses and what it
 * lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/activity-family", rules["activity-family"], {
  valid: [{ code: `<ActivityItem label="Files" selected />` }],
  invalid: [
    { code: `<button className="activity-tab">x</button>`, errors: [{ messageId: "oldClass" }] },
    {
      code: `<span className="activity-count activity-said">2</span>`,
      errors: [{ messageId: "oldClass" }, { messageId: "oldClass" }],
    },
    { code: `<div data-scope="activity">x</div>`, errors: [{ messageId: "scope" }] },
    { code: `<div data-scope="activity-bar">x</div>`, errors: [{ messageId: "scope" }] },
  ],
});
