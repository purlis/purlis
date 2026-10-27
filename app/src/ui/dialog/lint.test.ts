// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The dialog family's lint** (`lint.js` beside this file, #2904): what it refuses and what it
 * lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/dialog-family", rules["dialog-family"], {
  valid: [
    { code: `<div role="group">x</div>` },
    { code: `<Dialog onClose={close} title="Rename workspace" />` },
    // A role held in a value is a selector, not a dialog drawn by hand.
    { code: `<div data-role="dialog">x</div>` },
  ],
  invalid: [
    { code: `<div role="dialog">x</div>`, errors: [{ messageId: "raw" }] },
    {
      code: `<section role="alertdialog" aria-label="Quit">x</section>`,
      errors: [{ messageId: "raw" }],
    },
  ],
});
