// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The rail family's lint** (`lint.js` beside this file, #2915): what it refuses and what it
 * lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/rail-family", rules["rail-family"], {
  valid: [{ code: `<Rail aria-label="Projects"><RailProject name="purlis" /></Rail>` }],
  invalid: [
    {
      code: `<ul className="projects"><li className="project">x</li></ul>`,
      errors: [{ messageId: "oldClass" }, { messageId: "oldClass" }],
    },
    { code: `<span className="project-name">x</span>`, errors: [{ messageId: "oldClass" }] },
    { code: `<nav data-scope="rail">x</nav>`, errors: [{ messageId: "scope" }] },
  ],
});
