// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The tabs family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/tabs-family` (error: the session strip is on React Aria's tabs, #3042, and a page's
 * tabs are `PageTabs`, #3668): a `tab`, `tablist` or `tabpanel` role written by hand, a strip's
 * or a page's alike, in JSX or in an object of props, and `data-scope="tabs"` by hand. A tablist
 * is the family's `useTabStrip`, `TabStripRow` and `StripTab` (`ui/tabs/TabStrip`), or
 * `useTabStripRow` and `AsStripTab` where another family's container and button draw it (the
 * rail, an activity bar), a tab's cell its `TabCell`, and the panel a tab names `tabPanelOf`
 * (`ui/tabs/Tabs`). It is off in no file (#3134).
 */

import { classStrings } from "../familyLint.js";

/** The roles of a tablist's elements, which only the tabs family writes. */
const TAB_ROLES = new Set(["tab", "tablist", "tabpanel"]);

/** The family's scopes, which only its wrappers write: the strip's and a page's tabs. */
const SCOPES = new Set(["tabs", "page-tabs"]);

const TABS_SAID =
  "draw a page's tabs with `PageTabs` from `ui/tabs/PageTabs` (#3668), and a strip's with `useTabStrip`, `TabStripRow` and `StripTab` from `ui/tabs/TabStrip` (React Aria's tabs), a tab's cell with `TabCell` and the panel it names with `tabPanelOf` from `ui/tabs/Tabs` (#3042).";

const tabsFamily = {
  meta: {
    type: "problem",
    docs: { description: "A tablist is the shared tabs family, on React Aria, never by hand" },
    messages: {
      role: `\`role="{{role}}"\` by hand: ${TABS_SAID}`,
      scope: `\`data-scope="{{scope}}"\` is drawn by the family's wrappers only: ${TABS_SAID}`,
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
          SCOPES.has(node.value.value)
        )
          context.report({ node, messageId: "scope", data: { scope: node.value.value } });
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
          SCOPES.has(node.value.value)
        )
          context.report({ node, messageId: "scope", data: { scope: node.value.value } });
      },
    };
  },
};

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "tabs-family": tabsFamily,
};
