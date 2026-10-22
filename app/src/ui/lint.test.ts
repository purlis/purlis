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

/** The families a raw button may still belong to, as `eslint.config.js` names some of them. */
const options = [{ otherFamilies: { roles: ["tab", "menuitem"], classes: ["ui-row", "crumb"] } }];

tester.run("purlis/button-family", purlisUi.rules["button-family"], {
  valid: [
    { code: `<Button variant="primary" className="notice-go">Go</Button>` },
    { code: `<div className="answer">x</div>` },
    { code: `<li className="ui-row">x</li>` },
    // Another family's control, by its role or its family's class.
    { code: `<button role="tab" aria-selected>Chats</button>`, options },
    { code: `<button className="ui-row vault-secret">API_TOKEN</button>`, options },
    { code: `<button className={at === 0 ? "crumb own" : "crumb between"}>x</button>`, options },
  ],
  invalid: [
    {
      code: `<button className="ui-button">Save</button>`,
      errors: [{ messageId: "raw" }, { messageId: "oldClass" }],
    },
    {
      code: `<Button className="ui-button-primary">Save</Button>`,
      errors: [{ messageId: "oldClass" }],
    },
    {
      code: "<a className={`go ${on ? 'ui-button-ghost' : ''}`}>Go</a>",
      errors: [{ messageId: "oldClass" }],
    },
    {
      code: `<div className={clsx("x", on && "ui-button-sm")}>Go</div>`,
      errors: [{ messageId: "oldClass" }],
    },
    {
      code: `<div className={["ui-button-row", "x"].join(" ")}>Go</div>`,
      errors: [{ messageId: "oldClass" }],
    },
    // An action drawn by hand is an error now the button family has moved.
    { code: `<button onClick={go}>Delete</button>`, options, errors: [{ messageId: "raw" }] },
    {
      code: `<button className="tab-close" onClick={go}>×</button>`,
      options,
      errors: [{ messageId: "raw" }],
    },
  ],
});

tester.run("purlis/field-family", purlisUi.rules["field-family"], {
  valid: [
    { code: `<TextField label="Name" value={name} onChange={setName} />` },
    { code: `<TextArea aria-label="Your answer" value={text} onChange={setText} />` },
    // Another family's input, and one that draws nothing.
    { code: `<input type="checkbox" className="m-0" />` },
    { code: `<input type="range" min={1} max={9} />` },
    { code: `<input type="color" />` },
    { code: `<input type="hidden" name="at" value={at} />` },
    // The palette's box is a combobox's, which the combobox family moves.
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

tester.run("purlis/select-family", purlisUi.rules["select-family"], {
  valid: [{ code: `<Select value={x} onValueChange={setX} />` }],
  invalid: [
    { code: `<select value={x} />`, errors: [{ messageId: "unmoved" }] },
    { code: `<input role="combobox" />`, errors: [{ messageId: "unmoved" }] },
  ],
});

tester.run("purlis/choice-family", purlisUi.rules["choice-family"], {
  valid: [{ code: `<input type="hidden" />` }, { code: `<TextField value="" onChange={go} />` }],
  invalid: [
    { code: `<input type="checkbox" />`, errors: [{ messageId: "unmoved" }] },
    { code: `<input type="range" />`, errors: [{ messageId: "unmoved" }] },
    { code: `<input type="color" />`, errors: [{ messageId: "unmoved" }] },
  ],
});

tester.run("purlis/dialog-family", purlisUi.rules["dialog-family"], {
  valid: [{ code: `<div role="group">x</div>` }],
  invalid: [{ code: `<div role="dialog">x</div>`, errors: [{ messageId: "unmoved" }] }],
});

tester.run("purlis/tooltip-family", purlisUi.rules["tooltip-family"], {
  valid: [{ code: `<Field title="x" />` }],
  invalid: [
    { code: `<span title="Saved">x</span>`, errors: [{ messageId: "unmoved" }] },
    { code: `<Button title="Saves">Save</Button>`, errors: [{ messageId: "unmoved" }] },
  ],
});
