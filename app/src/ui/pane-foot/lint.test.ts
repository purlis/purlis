// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** **The pane foot's lint** (`lint.js` beside this file, #5202). */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/pane-foot-family", rules["pane-foot-family"], {
  valid: [
    { code: `<PaneFoot><PaneFootStart>{where}</PaneFootStart></PaneFoot>` },
    { code: `<div className="pane-body">{terminal}</div>` },
  ],
  invalid: [
    { code: `<div data-scope="pane-foot" data-part="root">x</div>`, errors: [{ messageId: "scope" }] },
  ],
});
