// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
/**
 * **The component layer's lint** (`eslint.ui-rules.js`, #2904): what it refuses, what it warns
 * of, and what it lets through, on code a screen would really write.
 */
import { RuleTester } from "eslint";
import tseslint from "typescript-eslint";
import { describe, it } from "vitest";
import { purlisUi } from "../../eslint.ui-rules.js";

RuleTester.describe = describe;
RuleTester.it = it;
RuleTester.itOnly = it.only;

const tester = new RuleTester({
  languageOptions: {
    parser: tseslint.parser,
    parserOptions: { ecmaFeatures: { jsx: true } },
  },
});

tester.run("purlis/button-family", purlisUi.rules["button-family"], {
  valid: [
    { code: `<Button variant="primary" className="notice-go">Go</Button>` },
    { code: `<div className="answer">x</div>` },
    { code: `<li className="ui-row">x</li>` },
  ],
  invalid: [
    { code: `<button className="ui-button">Save</button>`, errors: [{ messageId: "oldClass" }] },
    {
      code: `<Button className="ui-button-primary">Save</Button>`,
      errors: [{ messageId: "oldClass" }],
    },
    {
      code: "<a className={`go ${on ? 'ui-button-ghost' : ''}`}>Go</a>",
      errors: [{ messageId: "oldClass" }],
    },
    {
      code: `<button className={clsx("x", on && "ui-button-sm")}>Go</button>`,
      errors: [{ messageId: "oldClass" }],
    },
    {
      code: `<button className={["ui-button", "x"].join(" ")}>Go</button>`,
      errors: [{ messageId: "oldClass" }],
    },
  ],
});

tester.run("purlis/unmigrated-control", purlisUi.rules["unmigrated-control"], {
  valid: [
    { code: `<Button onPress={go}>Go</Button>` },
    { code: `<div role="group">x</div>` },
    { code: `<Field title="x" />` },
  ],
  invalid: [
    { code: `<button className="ui-row">Row</button>`, errors: [{ messageId: "raw" }] },
    { code: `<input className="ui-input" />`, errors: [{ messageId: "raw" }] },
    { code: `<div role="dialog">x</div>`, errors: [{ messageId: "dialog" }] },
    { code: `<span title="Saved">x</span>`, errors: [{ messageId: "title" }] },
    { code: `<Button title="Saves">Save</Button>`, errors: [{ messageId: "title" }] },
  ],
});
