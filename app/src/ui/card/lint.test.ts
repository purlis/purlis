// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The card family's lint** (`lint.js` beside this file, #2904): what it refuses and what it
 * lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/card-family", rules["card-family"], {
  valid: [
    { code: `<Card variant="ask" className="inbox-ask">x</Card>` },
    { code: `<div className="row-card">x</div>` },
  ],
  invalid: [
    {
      code: `<div className="block-report ui-inline-surface">x</div>`,
      errors: [{ messageId: "oldClass" }],
    },
    {
      code: `<p className="ui-inline-surface ui-inline-surface-waiting">x</p>`,
      errors: [{ messageId: "oldClass" }, { messageId: "oldClass" }],
    },
    { code: `<li data-scope="card">x</li>`, errors: [{ messageId: "scope" }] },
  ],
});
