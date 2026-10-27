// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The titlebar family's lint** (`lint.js` beside this file, #2904): what it refuses and what it
 * lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/titlebar-family", rules["titlebar-family"], {
  valid: [
    { code: `<WorkspaceChip name="alpha" current onClick={go} />` },
    { code: `<span className="workspace-settings">x</span>` },
  ],
  invalid: [
    { code: `<div className="workspaces-strip">x</div>`, errors: [{ messageId: "oldClass" }] },
    { code: `<span className="workspace">x</span>`, errors: [{ messageId: "oldClass" }] },
    { code: `<button className="plane-root">x</button>`, errors: [{ messageId: "oldClass" }] },
    { code: `<span data-scope="titlebar">x</span>`, errors: [{ messageId: "scope" }] },
  ],
});
