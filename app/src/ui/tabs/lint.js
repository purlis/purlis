// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The tabs family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/tabs-family` (error: the session strip is on React Aria's tabs, #3042): a `tab`,
 * `tablist` or `tabpanel` role written by hand, in JSX or in an object of props, the strip's
 * old hand-made tablist (`StripTablist`), and `data-scope="tabs"` by hand. A tablist is the
 * family's `useTabStrip`, `TabStripRow` and `StripTab` (`ui/tabs/TabStrip`), a tab's cell its
 * `TabCell`, and the panel a tab names `tabPanelOf` (`ui/tabs/Tabs`). The tablists still drawn
 * by hand are turned off by file below ({@link off}), each naming the ticket that moves it.
 */

import { classStrings } from "../familyLint.js";

/** The roles of a tablist's elements, which only the tabs family writes. */
const TAB_ROLES = new Set(["tab", "tablist", "tabpanel"]);

const TABS_SAID =
  "draw a tablist with `useTabStrip`, `TabStripRow` and `StripTab` from `ui/tabs/TabStrip` (React Aria's tabs), a tab's cell with `TabCell` and the panel it names with `tabPanelOf` from `ui/tabs/Tabs` (#3042).";

const tabsFamily = {
  meta: {
    type: "problem",
    docs: { description: "A tablist is the shared tabs family, on React Aria, never by hand" },
    messages: {
      role: `\`role="{{role}}"\` by hand: ${TABS_SAID}`,
      owner: `\`StripTablist\` is the strip's old hand-made tablist: ${TABS_SAID}`,
      scope: `\`data-scope="tabs"\` is drawn by the family's wrappers only: ${TABS_SAID}`,
    },
    schema: [],
  },
  create(context) {
    /** Reports `node` when `value` is a literal tab role. */
    // Every string the role can hold: a literal, `{"tab"}`, a template, either branch.
    const role = (node, value) => {
      for (const [, text] of classStrings(value))
        if (TAB_ROLES.has(text.trim()))
          context.report({ node, messageId: "role", data: { role: text.trim() } });
    };
    return {
      JSXAttribute(node) {
        if (node.name.name === "role") role(node, node.value);
        else if (
          node.name.name === "data-scope" &&
          node.value?.type === "Literal" &&
          node.value.value === "tabs"
        )
          context.report({ node, messageId: "scope" });
      },
      // `{ role: "tabpanel", … }`, handed on to an element.
      Property(node) {
        const key =
          node.key.type === "Identifier"
            ? node.key.name
            : node.key.type === "Literal"
              ? node.key.value
              : undefined;
        if (key === "role") role(node, node.value);
        else if (
          key === "data-scope" &&
          node.value?.type === "Literal" &&
          node.value.value === "tabs"
        )
          context.report({ node, messageId: "scope" });
      },
      ImportDeclaration(node) {
        if (!/(?:^|\/)StripTablist$/.test(String(node.source.value))) return;
        for (const one of node.specifiers)
          if (one.type === "ImportSpecifier" && one.imported.name === "StripTablist")
            context.report({ node: one, messageId: "owner" });
      },
    };
  },
};

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "tabs-family": tabsFamily,
};

/**
 * **The tablists still drawn by hand**, each until the ticket that moves it onto the tabs
 * family (#3042): the workspace strip (#2916), the activity bars (#2917), the project rail's
 * tablist (#2915's rail family, whose projects are tabs), and the strip's old hand-made tablist
 * they share. `eslint.config.js` turns the family's rule off in each.
 */
export const off = {
  "tabs-family": [
    "src/WorkspaceStrip.tsx",
    "src/ActivityBar.tsx",
    "src/ProjectRail.tsx",
    "src/StripTablist.tsx",
  ],
};
