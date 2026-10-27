// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The tooltip family's lint** (`lint.js` beside this file, #2904): what it refuses and what it
 * lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/tooltip-family", rules["tooltip-family"], {
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
