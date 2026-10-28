// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The tabs family's lint** (`lint.js` beside this file, #2904): what it refuses and what it
 * lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/tabs-family", rules["tabs-family"], {
  valid: [
    {
      code: `<StripTab strip={along} tabKey="1" selected tip={undefined} id={idOf(0)}>talk</StripTab>`,
    },
    { code: `<TabStripRow strip={along} name="Tabs" ids={ids} className="tabs">x</TabStripRow>` },
    { code: `<div className="panes" {...tabPanelOf(namedBy)} />` },
    // Another role, and a selector that reads a tab: neither draws one.
    { code: `<li role="treeitem">x</li>` },
    { code: `<li role={open ? "treeitem" : "option"}>x</li>` },
    { code: `strip.querySelector('[role="tab"][aria-selected="true"]')` },
    // The id helper beside the old tablist is not a tablist.
    { code: `import { useTabIds } from "./StripTablist";` },
    { code: `<span data-scope="rail">x</span>` },
    // A page's tabs, drawn by the family.
    {
      code: `<PageTabs label="deploy's sections" tabs={tabs} selected={tab} onSelect={setTab}>{panel}</PageTabs>`,
    },
  ],
  invalid: [
    { code: `<button role="tab" aria-selected>talk</button>`, errors: [{ messageId: "role" }] },
    { code: `<div role="tablist" aria-label="Tabs" />`, errors: [{ messageId: "role" }] },
    { code: `<div role="tabpanel" aria-labelledby={id} />`, errors: [{ messageId: "role" }] },
    { code: `<button role={"tab"}>talk</button>`, errors: [{ messageId: "role" }] },
    { code: "<div role={`tablist`} />", errors: [{ messageId: "role" }] },
    { code: `<li role={open ? "tab" : "option"} />`, errors: [{ messageId: "role" }] },
    {
      code: `const panel = { role: "tabpanel", "aria-labelledby": id };`,
      errors: [{ messageId: "role" }],
    },
    {
      code: `import { StripTablist, useTabIds } from "./StripTablist";`,
      errors: [{ messageId: "owner" }],
    },
    { code: `<span data-scope="tabs" data-part="tab" />`, errors: [{ messageId: "scope" }] },
    // A page's tablist written by hand, as a skill page would (#3668).
    {
      code: `<div role="tablist" aria-label="deploy's sections"><button role="tab" aria-selected>Overview</button></div>`,
      errors: [{ messageId: "role" }, { messageId: "role" }],
    },
    { code: `<div data-scope="page-tabs" data-part="list" />`, errors: [{ messageId: "scope" }] },
    {
      code: `const part = { "data-scope": "tabs", "data-part": "strip" };`,
      errors: [{ messageId: "scope" }],
    },
  ],
});
