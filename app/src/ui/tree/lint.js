// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The tree family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/tree-family` (error: a tree is the tree family's, #2911): a tree's role written by
 * hand (`tree`, `treeitem`, `treegrid`), the shared tree's old classes (`ui-tree`,
 * `ui-tree-row`, `ui-tree-twist`) and its `data-scope`. A tree is the family's `Tree` and
 * `TreeRow` (`ui/tree/Tree`). The trees still drawn by hand are turned off by file below
 * ({@link off}), each until its lane moves it.
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

/**
 * **The trees still drawn by hand** (the shared tree's classes and roles), each until its lane
 * moves it onto the tree family (#2911): the Chats list with the browsers and finished tasks
 * under a chat (#3198). `eslint.config.js` turns the family's rule off in each.
 */
export const off = {
  "tree-family": ["src/ChatsSection.tsx", "src/ChatBrowsersShown.tsx", "src/FinishedTasks.tsx"],
};

/**
 * **This family's controls still drawn as a raw `<button>`** (D-2904-9): A tree's row drawn by hand, said by its role
 * or its class, which the button family's rule lets through until this family's lane moves them.
 */
export const rawButtons = {
  roles: ["treeitem"],
  classes: ["ui-tree-row"],
};
