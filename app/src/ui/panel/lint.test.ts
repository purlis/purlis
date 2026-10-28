// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The panel family's lint** (`lint.js` beside this file, #2904): what it refuses and what it
 * lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/panel-family", rules["panel-family"], {
  valid: [
    { code: `<Panel variant="side" className="region-slot">x</Panel>` },
    { code: `<Group {...panelRoot("content")} className="region-centre">x</Group>` },
  ],
  invalid: [{ code: `<div data-scope="panel">x</div>`, errors: [{ messageId: "scope" }] }],
});

tester.run("purlis/panel-section-family", rules["panel-section-family"], {
  valid: [{ code: `<PanelSectionTitle className="inbox-chain">Updates</PanelSectionTitle>` }],
  invalid: [
    {
      code: `<h2 className="sidebar-title ui-text-label">Chats</h2>`,
      errors: [{ messageId: "oldClass" }],
    },
    {
      code: `<div className={on ? "panel-head x" : "x"}>x</div>`,
      errors: [{ messageId: "oldClass" }],
    },
    { code: `<h2 data-scope="panel-section">x</h2>`, errors: [{ messageId: "scope" }] },
  ],
});

tester.run("purlis/panel-separator-family", rules["panel-separator-family"], {
  valid: [
    { code: `<Separator aria-label="Resize the split" />` },
    { code: `import { Group, Panel } from "react-resizable-panels";` },
    { code: `import { Separator } from "./ui/panel/Separator";` },
  ],
  invalid: [
    {
      code: `<div data-scope="panel-separator" role="separator" />`,
      errors: [{ messageId: "scope" }],
    },
    {
      code: `import { Group, Separator } from "react-resizable-panels";`,
      errors: [{ messageId: "library" }],
    },
    {
      code: `import { Separator as Handle } from "react-resizable-panels";`,
      errors: [{ messageId: "library" }],
    },
    {
      code: `import * as Panels from "react-resizable-panels";`,
      errors: [{ messageId: "library" }],
    },
  ],
});

tester.run("purlis/view-frame-family", rules["view-frame-family"], {
  valid: [
    {
      code: `<ViewFrame label="Studio pages" pages={pages} page="library" onPage={go} list={x} />`,
    },
    { code: `<ViewPageHeader name="deploy" description="Deploys a service." />` },
  ],
  invalid: [
    {
      code: `<div data-scope="view-frame" data-part="nav">x</div>`,
      errors: [{ messageId: "scope" }],
    },
    {
      code: `<button data-scope="view-nav" data-part="item">Library</button>`,
      errors: [{ messageId: "scope" }],
    },
  ],
});
