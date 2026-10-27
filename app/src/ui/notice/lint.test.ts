// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The notice family's lint** (`lint.js` beside this file, #2904): what it refuses and what it
 * lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/notice-family", rules["notice-family"], {
  valid: [
    { code: `<Notice cause="x" onDismiss={go}>x</Notice>` },
    { code: `<section className="inbox-notices specimen-notice">x</section>` },
  ],
  invalid: [
    {
      code: `<div className="notice notice-pane notice-trouble">x</div>`,
      errors: [{ messageId: "oldClass" }, { messageId: "oldClass" }, { messageId: "oldClass" }],
    },
    {
      code: `<div className={empty ? "pane-notice-row pane-notice-row-empty" : "pane-notice-row"} />`,
      errors: [{ messageId: "oldClass" }, { messageId: "oldClass" }, { messageId: "oldClass" }],
    },
    { code: `<div data-scope="notice-list" />`, errors: [{ messageId: "scope" }] },
  ],
});
