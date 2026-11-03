// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** **The code family's lint** (`lint.js` beside this file, #4481). */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/code-family", rules["code-family"], {
  valid: [{ code: `<CodeBlock label="Commands" text="ls" />` }, { code: `<code>{text}</code>` }],
  invalid: [{ code: `<span data-scope="code">x</span>`, errors: [{ messageId: "scope" }] }],
});
