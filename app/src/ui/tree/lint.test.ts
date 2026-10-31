// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The tree family's lint** (`lint.js` beside this file, #2904): what it refuses and what it
 * lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/tree-family", rules["tree-family"], {
  valid: [
    { code: `<Tree aria-label="Repos of alpha" size="sm" fixed>{rows}</Tree>` },
    { code: `<TreeRow id="src" textValue="src" content={<RowLabel>src</RowLabel>} />` },
    { code: `<div className="tree-held">x</div>` },
  ],
  invalid: [
    { code: `<div role="tree" aria-label="Files">{rows}</div>`, errors: [{ messageId: "role" }] },
    { code: `<li role={"treeitem"}>x</li>`, errors: [{ messageId: "role" }] },
    { code: `<div role="treegrid">{rows}</div>`, errors: [{ messageId: "role" }] },
    { code: `<ul className="ui-tree">{rows}</ul>`, errors: [{ messageId: "oldClass" }] },
    {
      code: `<button className="ui-row ui-tree-row">x</button>`,
      errors: [{ messageId: "oldClass" }],
    },
    { code: `<span className="ui-tree-twist" />`, errors: [{ messageId: "oldClass" }] },
    { code: `<div data-scope="tree">x</div>`, errors: [{ messageId: "scope" }] },
  ],
});
