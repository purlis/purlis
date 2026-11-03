// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** **The checks family's lint** (`lint.js` beside this file, #4481). */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/checks-family", rules["checks-family"], {
  valid: [
    { code: `<CheckList aria-label="Checks"><CheckLine verdict="ok" title="x" /></CheckList>` },
    { code: `<ul>{lines}</ul>` },
  ],
  invalid: [{ code: `<span data-scope="checks">x</span>`, errors: [{ messageId: "scope" }] }],
});
