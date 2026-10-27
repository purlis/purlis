// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The select family's lint** (`lint.js` beside this file, #2904): what it refuses and what it
 * lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/select-family", rules["select-family"], {
  valid: [
    { code: `<Select aria-label="Kind" value={x} options={kinds} onValueChange={setX} />` },
    { code: `<ComboBox label="Add a skill" value={x} suggestions={known} onChange={setX} />` },
    { code: `<ListComboBox controls={["rows"]} aimed={at} value={q} onChange={setQ} />` },
    // A role held in a value is a selector, not a combobox drawn by hand.
    { code: `<div data-role="combobox">x</div>` },
    { code: `<button aria-haspopup="menu">More</button>` },
    // A role decided at run time is no hand-written combobox.
    { code: `<div role={kind} />` },
  ],
  invalid: [
    { code: `<select value={x} />`, errors: [{ messageId: "raw" }] },
    { code: `<datalist id="known" />`, errors: [{ messageId: "raw" }] },
    { code: `<input role="combobox" />`, errors: [{ messageId: "role" }] },
    { code: `<TextField role="combobox" value={q} />`, errors: [{ messageId: "role" }] },
    { code: `<div aria-haspopup="listbox">Pick</div>`, errors: [{ messageId: "role" }] },
    { code: `<input role={"combobox"} />`, errors: [{ messageId: "role" }] },
    { code: "<div role={`combobox`} />", errors: [{ messageId: "role" }] },
    { code: `<div aria-haspopup={"listbox"}>Pick</div>`, errors: [{ messageId: "role" }] },
    { code: `<button className="ui-select wide" />`, errors: [{ messageId: "oldClass" }] },
  ],
});
