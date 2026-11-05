// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** **The pane preview's lint** (`lint.js` beside this file, #5291). */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/pane-preview-family", rules["pane-preview-family"], {
  valid: [
    { code: `<PanePreviewRefused box={box} />` },
    { code: `<div className="pane-body">{terminal}</div>` },
  ],
  invalid: [
    {
      code: `<div data-scope="pane-preview" data-part="size">x</div>`,
      errors: [{ messageId: "scope" }],
    },
  ],
});
