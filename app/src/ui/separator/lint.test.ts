// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The separator family's lint** (`lint.js` beside this file, #2904): what it refuses and what it
 * lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/separator-family", rules["separator-family"], {
  valid: [{ code: `<Separator decorative className="activity-divider" />` }],
  invalid: [
    { code: `<hr />`, errors: [{ messageId: "element" }] },
    { code: `<div data-scope="separator" />`, errors: [{ messageId: "scope" }] },
  ],
});
