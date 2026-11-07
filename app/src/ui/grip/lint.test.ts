// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The grip family's lint** (`lint.js` beside this file, #5290): what it refuses and what it lets
 * through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/grip-family", rules["grip-family"], {
  valid: [{ code: `<Grip label="Move chat 1" onPress={pickUp}>{mark}</Grip>` }],
  invalid: [
    { code: `<span data-scope="grip" data-part="root" />`, errors: [{ messageId: "scope" }] },
    { code: `<span className="chat-row-place" />`, errors: [{ messageId: "oldClass" }] },
  ],
});
