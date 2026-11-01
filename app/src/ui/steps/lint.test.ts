// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** **The steps family's lint** (`lint.js` beside this file, #4480). */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/steps-family", rules["steps-family"], {
  valid: [
    { code: `<StepList aria-label="Steps" steps={steps} showing="pool" onGo={go} />` },
    { code: `<ol>{steps}</ol>` },
  ],
  invalid: [{ code: `<span data-scope="steps">x</span>`, errors: [{ messageId: "scope" }] }],
});
