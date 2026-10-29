// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The review page's lint** (`lint.js` beside this file, #3672): what it refuses and what it
 * lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/review-page-family", rules["review-page-family"], {
  valid: [
    {
      code: `<ReviewPage label="Install deploy" summary="Nothing is installed until you press Install." onCancel={back} main={{ children: "Install", onPress: install }}>{files}</ReviewPage>`,
    },
  ],
  invalid: [
    { code: `<div data-scope="review-page" data-part="bar" />`, errors: [{ messageId: "scope" }] },
  ],
});
