// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The status family's lint** (`lint.js` beside this file, #2904): what it refuses and what it
 * lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/status-item-family", rules["status-item-family"], {
  valid: [
    { code: `<StatusItem variant="pill" onPress={go}>About</StatusItem>` },
    { code: `<span className="title-kill-switch-trouble">x</span>` },
    { code: `<span className="save-indicator-words">Saved</span>` },
  ],
  invalid: [
    { code: `<footer className="status-line">x</footer>`, errors: [{ messageId: "oldClass" }] },
    {
      code: `<button className="needs-you-button">x</button>`,
      errors: [{ messageId: "oldClass" }],
    },
    {
      code: "<b className={`status-doctor doctor-tone-${tone}`}>x</b>",
      errors: [{ messageId: "oldClass" }, { messageId: "oldClass" }],
    },
    { code: `<span data-scope="status-item">x</span>`, errors: [{ messageId: "scope" }] },
  ],
});
