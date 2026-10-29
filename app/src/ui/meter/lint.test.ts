// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The meter family's lint** (`lint.js` beside this file, #3674): what it refuses and what it
 * lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/meter-family", rules["meter-family"], {
  valid: [
    { code: `<Meter label="With it" value={86} valueLabel="86%" />` },
    { code: `<ProgressBar label="Chats" value={3} maxValue={12} valueLabel="3 of 12" />` },
    { code: `<div role="group" aria-label="Latest grade">{meters}</div>` },
  ],
  invalid: [
    { code: `<progress value={45} max={100} />`, errors: [{ messageId: "element" }] },
    { code: `<meter value={0.8} />`, errors: [{ messageId: "element" }] },
    { code: `<div role="progressbar" aria-valuenow={3}>x</div>`, errors: [{ messageId: "role" }] },
    { code: `<div role={"meter"}>x</div>`, errors: [{ messageId: "role" }] },
    { code: `<span data-scope="meter">x</span>`, errors: [{ messageId: "scope" }] },
  ],
});
