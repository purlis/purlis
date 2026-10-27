// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The badge family's lint** (`lint.js` beside this file, #2904): what it refuses and what it
 * lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/badge-family", rules["badge-family"], {
  valid: [
    { code: `<Badge variant="waiting">Waiting for you</Badge>` },
    { code: `<Count className="activity-count" variant="hot" said="2 need you">2</Count>` },
  ],
  invalid: [
    { code: `<span className="ui-setting-badge">x</span>`, errors: [{ messageId: "oldClass" }] },
    {
      code: `<span className="count-pill"><span className="count-pill-needs">2</span></span>`,
      errors: [{ messageId: "oldClass" }, { messageId: "oldClass" }],
    },
    { code: `<span data-scope="count">2</span>`, errors: [{ messageId: "scope" }] },
  ],
});
