// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The list family's lint** (`lint.js` beside this file, #2904): what it refuses and what it
 * lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/list-family", rules["list-family"], {
  valid: [
    { code: `<ListBox aria-label="Personas" selectionMode="single">{rows}</ListBox>` },
    { code: `<List aria-label="Todos" size="sm">{rows}</List>` },
    // A list of words is no collection: a sentence's bullets stay a list.
    { code: `<ul className="contributes"><li>a theme</li></ul>` },
    { code: `<div role="list"><span role="listitem">x</span></div>` },
  ],
  invalid: [
    {
      code: `<div role="listbox" aria-label="Search results">{hits}</div>`,
      errors: [{ messageId: "role" }],
    },
    { code: `<li role="option" aria-selected>x</li>`, errors: [{ messageId: "role" }] },
    { code: `<ul role={"listbox"}>{rows}</ul>`, errors: [{ messageId: "role" }] },
    { code: `<li role={on ? "option" : "none"}>x</li>`, errors: [{ messageId: "role" }] },
    { code: `<ul className="ui-list">{rows}</ul>`, errors: [{ messageId: "oldClass" }] },
    { code: `<div data-scope="list">x</div>`, errors: [{ messageId: "scope" }] },
  ],
});
