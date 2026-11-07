// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Secret ask's lint** (`lint.js` beside this file, #5080): what it refuses and what it lets
 * through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/secret-ask-family", rules["secret-ask-family"], {
  valid: [
    {
      code: `<SecretAskFacts><SecretAskLine who="Program">{path}</SecretAskLine></SecretAskFacts>`,
    },
  ],
  invalid: [
    { code: `<div data-scope="secret-ask" data-part="facts" />`, errors: [{ messageId: "scope" }] },
  ],
});
