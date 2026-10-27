// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The field family's lint** (`lint.js` beside this file, #2904): what it refuses and what it
 * lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/field-family", rules["field-family"], {
  valid: [
    { code: `<TextField label="Name" value={name} onChange={setName} />` },
    { code: `<TextArea aria-label="Your answer" value={text} onChange={setText} />` },
    // Another family's input, and one that draws nothing.
    { code: `<input type="checkbox" className="m-0" />` },
    { code: `<input type="range" min={1} max={9} />` },
    { code: `<input type="color" />` },
    { code: `<input type="hidden" name="at" value={at} />` },
    // A combobox's box is the select family's rule's to refuse.
    { code: `<input role="combobox" aria-expanded="true" />` },
  ],
  invalid: [
    { code: `<input />`, errors: [{ messageId: "raw" }] },
    { code: `<input type="search" aria-label="Filter" />`, errors: [{ messageId: "raw" }] },
    { code: `<input type={shown ? "text" : "password"} />`, errors: [{ messageId: "raw" }] },
    { code: `<textarea />`, errors: [{ messageId: "raw" }] },
    {
      code: `<input className="ui-input" />`,
      errors: [{ messageId: "raw" }, { messageId: "oldClass" }],
    },
    { code: `<span className="box ui-input-lead">x</span>`, errors: [{ messageId: "oldClass" }] },
    { code: `<Search className="ui-input-lead-icon" />`, errors: [{ messageId: "oldClass" }] },
  ],
});
