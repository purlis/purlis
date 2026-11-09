// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The tree family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/tree-family` (error: a tree is the tree family's, #2911): a tree's role written by
 * hand (`tree`, `treeitem`, `treegrid`), the shared tree's old classes (`ui-tree`,
 * `ui-tree-row`, `ui-tree-twist`) and its `data-scope`. A tree is the family's `Tree` and
 * `TreeRow` (`ui/tree/Tree`). No tree is drawn by hand any more: the Chats list was the last
 * (#3198), so the rule is off nowhere.
 */

import { moved } from "../familyLint.js";

const treeFamily = moved({
  description: "A tree is the shared tree family",
  scopes: ["tree"],
  classes: ["ui-tree", "ui-tree-row", "ui-tree-twist"],
  roles: ["tree", "treeitem", "treegrid"],
  message:
    "a tree: draw it as `<Tree>` and `<TreeRow>`, with the row's slots, from `ui/tree/Tree` (#2911).",
});

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "tree-family": treeFamily,
};
