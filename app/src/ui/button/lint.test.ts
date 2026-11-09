// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The button family's lint** (`lint.js` beside this file, #2904): what it refuses and what it
 * lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/button-family", rules["button-family"], {
  valid: [
    { code: `<Button variant="primary" className="notice-go">Go</Button>` },
    { code: `<div className="answer">x</div>` },
    { code: `<li className="ui-row">x</li>` },
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
    { code: `<button onClick={go}>Delete</button>`, errors: [{ messageId: "raw" }] },
    {
      code: `<button className="tab-close" onClick={go}>×</button>`,
      errors: [{ messageId: "raw" }],
    },
    // No role or class lets a raw button through any more (#2930), not even a family's own.
    { code: `<button role="tab" aria-selected>Chats</button>`, errors: [{ messageId: "raw" }] },
    {
      code: `<button className={clsx("ui-tabs-control closer", { keeps })}>×</button>`,
      errors: [{ messageId: "raw" }],
    },
    { code: `<button className="bare">x</button>`, errors: [{ messageId: "raw" }] },
    { code: `<button className="closer">×</button>`, errors: [{ messageId: "raw" }] },
  ],
});
