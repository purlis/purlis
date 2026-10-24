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

tester.run("purlis/menu-family", purlisUi.rules["menu-family"], {
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
  valid: [
    { code: `<div role="group">x</div>` },
    { code: `<Dialog onClose={close} title="Rename workspace" />` },
    // A role held in a value is a selector, not a dialog drawn by hand.
    { code: `<div data-role="dialog">x</div>` },
  ],
  invalid: [
    { code: `<div role="dialog">x</div>`, errors: [{ messageId: "raw" }] },
    {
      code: `<section role="alertdialog" aria-label="Quit">x</section>`,
      errors: [{ messageId: "raw" }],
    },
  ],
});

tester.run("purlis/tooltip-family", purlisUi.rules["tooltip-family"], {
  valid: [
    // A component's own `title` prop is its heading, not a tooltip.
    { code: `<Field title="x" />` },
    { code: `<Tooltip tip="Saved at 10:02"><span>Saved</span></Tooltip>` },
    { code: `<Button tooltip="Saves every file">Save</Button>` },
    { code: `<IconButton aria-label="Close" icon={<X />} />` },
    // A frame's title is its name.
    { code: `<iframe title="The page" src={url} />` },
    // A spread with no title, and a component's own props.
    { code: `function Row() { const shared = { disabled: off }; return <button {...shared} />; }` },
    { code: `<Field {...{ title: "Depth" }} />` },
  ],
  invalid: [
    { code: `<span title="Saved">x</span>`, errors: [{ messageId: "title" }] },
    { code: `<button role="tab" title={path}>x</button>`, errors: [{ messageId: "title" }] },
    { code: `<Button title="Saves">Save</Button>`, errors: [{ messageId: "title" }] },
    {
      code: `<IconButton aria-label="Close" title="Close" icon={<X />} />`,
      errors: [{ messageId: "title" }],
    },
    // A primitive's part hands it on to its element.
    { code: `<RadioGroup.Item value="a" title="Initials" />`, errors: [{ messageId: "title" }] },
    // Through a spread of an object written in place, or of a `const` in scope.
    { code: `<button {...{ title: why }}>x</button>`, errors: [{ messageId: "title" }] },
    {
      code: `function Row() { const shared = { disabled: off, title: why }; return <Menu.Item {...shared} />; }`,
      errors: [{ messageId: "title" }],
    },
    // An SVG's own title is a native tooltip too.
    { code: `<svg><rect><title>turn 1</title></rect></svg>`, errors: [{ messageId: "svgTitle" }] },
    // A house component that hands it on, as `eslint.config.js` names them.
    {
      code: `<Select title={why} value={v} />`,
      options: [{ handsOn: ["Select"] }],
      errors: [{ messageId: "title" }],
    },
  ],
});

tester.run("purlis/panel-family", purlisUi.rules["panel-family"], {
  valid: [
    { code: `<Panel variant="side" className="region-slot">x</Panel>` },
    { code: `<Group {...panelRoot("content")} className="region-centre">x</Group>` },
  ],
  invalid: [{ code: `<div data-scope="panel">x</div>`, errors: [{ messageId: "scope" }] }],
});

tester.run("purlis/panel-section-family", purlisUi.rules["panel-section-family"], {
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

tester.run("purlis/empty-state-family", purlisUi.rules["empty-state-family"], {
  valid: [{ code: `<EmptyState headline="No todos" variant="panel" />` }],
  invalid: [
    {
      code: `<div className="empty-state empty-state-page">x</div>`,
      errors: [{ messageId: "oldClass" }, { messageId: "oldClass" }],
    },
    { code: `<p className="empty-state-headline">x</p>`, errors: [{ messageId: "oldClass" }] },
  ],
});

tester.run("purlis/status-item-family", purlisUi.rules["status-item-family"], {
  valid: [
    { code: `<StatusItem variant="pill" onPress={go}>About</StatusItem>` },
    { code: `<span className="title-kill-switch-trouble">x</span>` },
    { code: `<span className="save-indicator-words">Saved</span>` },
  ],
  invalid: [
    { code: `<footer className="status-line">x</footer>`, errors: [{ messageId: "oldClass" }] },
    {
      code: `<button className="needs-you-button">x</button>`,
      errors: [{ messageId: "oldClass" }],
    },
    {
      code: "<b className={`status-doctor doctor-tone-${tone}`}>x</b>",
      errors: [{ messageId: "oldClass" }, { messageId: "oldClass" }],
    },
    { code: `<span data-scope="status-item">x</span>`, errors: [{ messageId: "scope" }] },
  ],
});

tester.run("purlis/list-family", purlisUi.rules["list-family"], {
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

tester.run("purlis/tabs-family", purlisUi.rules["tabs-family"], {
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
    {
      code: `const part = { "data-scope": "tabs", "data-part": "strip" };`,
      errors: [{ messageId: "scope" }],
    },
  ],
});

tester.run("purlis/titlebar-family", purlisUi.rules["titlebar-family"], {
  valid: [
    { code: `<WorkspaceChip name="alpha" current onClick={go} />` },
    { code: `<span className="workspace-settings">x</span>` },
  ],
  invalid: [
    { code: `<div className="workspaces-strip">x</div>`, errors: [{ messageId: "oldClass" }] },
    { code: `<span className="workspace">x</span>`, errors: [{ messageId: "oldClass" }] },
    { code: `<button className="plane-root">x</button>`, errors: [{ messageId: "oldClass" }] },
    { code: `<span data-scope="titlebar">x</span>`, errors: [{ messageId: "scope" }] },
  ],
});
