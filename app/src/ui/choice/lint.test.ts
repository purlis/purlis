// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The choice family's lint** (`lint.js` beside this file, #2904): what it refuses and what it
 * lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/choice-family", rules["choice-family"], {
  valid: [
    { code: `<input type="hidden" />` },
    { code: `<TextField value="" onChange={go} />` },
    { code: `<Checkbox isSelected={on} onChange={setOn}>Sign commits</Checkbox>` },
    { code: `<SegmentedControl aria-label="Level" value={x} onChange={setX} />` },
    { code: `import { Switch } from "./ui/choice/Switch";` },
    // A menu's row that is ticked is the menu family's, and a tab is the tabs'.
    { code: `<div role="menuitemcheckbox">x</div>` },
    { code: `<button role="tab" aria-selected>Chats</button>` },
    // A role held in a value is a selector, not a choice drawn by hand.
    { code: `<div data-role="radio">x</div>` },
  ],
  invalid: [
    { code: `<input type="checkbox" />`, errors: [{ messageId: "raw" }] },
    { code: `<input type="radio" name="x" />`, errors: [{ messageId: "raw" }] },
    { code: `<input type="range" />`, errors: [{ messageId: "raw" }] },
    { code: `<input type="color" />`, errors: [{ messageId: "raw" }] },
    { code: `<input type="file" />`, errors: [{ messageId: "raw" }] },
    // An ad-hoc toggle: a role only a choice says, written by hand.
    {
      code: `<button role="switch" aria-checked={on}>x</button>`,
      errors: [{ messageId: "role" }, { messageId: "checked" }],
    },
    { code: `<div role="radiogroup">x</div>`, errors: [{ messageId: "role" }] },
    {
      code: `<span role="checkbox" aria-checked>x</span>`,
      errors: [{ messageId: "role" }, { messageId: "checked" }],
    },
    { code: `<button role={"switch"}>x</button>`, errors: [{ messageId: "role" }] },
    { code: `<div role={on ? "radio" : "button"}>x</div>`, errors: [{ messageId: "role" }] },
    { code: `<button aria-checked={on}>x</button>`, errors: [{ messageId: "checked" }] },
    {
      code: `import * as RadioGroup from "@radix-ui/react-radio-group";`,
      errors: [{ messageId: "oldImport" }],
    },
    {
      code: `import * as Switch from "@radix-ui/react-switch";`,
      errors: [{ messageId: "oldImport" }],
    },
    {
      code: `<div className="ui-levels ui-levels-small">x</div>`,
      errors: [{ messageId: "oldClass" }],
    },
    { code: `<span className={on ? "ui-switch" : ""} />`, errors: [{ messageId: "oldClass" }] },
    { code: `<div className="ui-choice-option">x</div>`, errors: [{ messageId: "oldClass" }] },
  ],
});
