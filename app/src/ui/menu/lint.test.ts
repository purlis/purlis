// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The menu family's lint** (`lint.js` beside this file, #2904): what it refuses and what it
 * lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/menu-family", rules["menu-family"], {
  valid: [
    {
      code: `<Menu aria-label="Tab steward 4"><MenuItem textValue="Rename…">Rename…</MenuItem></Menu>`,
    },
    { code: `<div role="group" className="tasks-menu">x</div>` },
    { code: `import { Menu } from "./ui/menu/Menu";` },
    { code: `import * as Tooltip from "./overlays/tooltip";` },
    { code: `<MenuItem textValue="End"><MenuWords said="End" /></MenuItem>` },
    { code: `<MenuItem>{chat.name}</MenuItem>` },
  ],
  invalid: [
    { code: `<span role="menu">x</span>`, errors: [{ messageId: "role" }] },
    { code: `<button role="menuitem">x</button>`, errors: [{ messageId: "role" }] },
    { code: `<div className="ui-menu more-menu">x</div>`, errors: [{ messageId: "oldClass" }] },
    {
      code: `<div className={danger ? "ui-menu-item ui-menu-item-danger" : "x"}>x</div>`,
      errors: [{ messageId: "oldClass" }],
    },
    { code: `<div className="ui-popover row-card">x</div>`, errors: [{ messageId: "oldClass" }] },
    {
      code: `import * as Menu from "@radix-ui/react-dropdown-menu";`,
      errors: [{ messageId: "oldImport" }],
    },
    {
      code: `import * as Popover from "./overlays/popover";`,
      errors: [{ messageId: "oldImport" }],
    },
    // A row that draws more than words says them, so typing finds it.
    {
      code: `<MenuItem><MenuWords said="End" /></MenuItem>`,
      errors: [{ messageId: "noTextValue" }],
    },
    // A grant is never a menu row: its lock and its edge are a button's (ADR 0097 §6).
    { code: `<MenuItem textValue="Allow network">x</MenuItem>`, errors: [{ messageId: "grants" }] },
    { code: `<MenuItem>Accept the change</MenuItem>`, errors: [{ messageId: "grants" }] },
    // Anywhere in the label, not only first.
    { code: `<MenuItem textValue="Always allow">x</MenuItem>`, errors: [{ messageId: "grants" }] },
    // An element drawn in braces is no words for typing to find.
    { code: `<MenuItem>{<Icon />}</MenuItem>`, errors: [{ messageId: "noTextValue" }] },
  ],
});
