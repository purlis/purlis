// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The empty-state family's lint** (`lint.js` beside this file, #2904): what it refuses and what it
 * lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/empty-state-family", rules["empty-state-family"], {
  valid: [{ code: `<EmptyState headline="No todos" variant="panel" />` }],
  invalid: [
    {
      code: `<div className="empty-state empty-state-page">x</div>`,
      errors: [{ messageId: "oldClass" }, { messageId: "oldClass" }],
    },
    { code: `<p className="empty-state-headline">x</p>`, errors: [{ messageId: "oldClass" }] },
  ],
});
