// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The disclosure family's lint** (`eslint.ui-rules.js` finds this file, #3675):
 *
 * `purlis/disclosure-family` (error): a fold's chevron swapped by hand, one chevron for open and
 * another for folded (`open ? <ChevronDown /> : <ChevronRight />`), and `data-scope="disclosure"`
 * by hand. A fold is the family's `Disclosure` (`ui/disclosure/Disclosure`); a fold that is not
 * one (a tree's group, a table's row, a menu's item) draws the family's `Twist`, the one chevron
 * that turns (#1916).
 */

import { moved } from "../familyLint.js";

const SAID =
  "draw a fold as `<Disclosure>`, or its chevron as `<Twist open={…} />`, the one chevron that turns, from `ui/disclosure/Disclosure` (#3675).";

const scoped = moved({
  description: "A fold is the shared disclosure family",
  scopes: ["disclosure"],
  classes: [],
  message: `a disclosure: ${SAID}`,
});

/** Whether `node` is a chevron icon drawn as an element (`<ChevronDown … />`). */
function chevron(node) {
  return (
    node?.type === "JSXElement" &&
    node.openingElement.name.type === "JSXIdentifier" &&
    /^Chevron(Up|Down|Left|Right)$/.test(node.openingElement.name.name)
  );
}

const disclosureFamily = {
  meta: {
    type: "problem",
    docs: { description: "A fold is the shared disclosure family, its chevron one that turns" },
    messages: {
      ...scoped.meta.messages,
      swap: `A fold's chevron swapped for another by hand: ${SAID}`,
    },
    schema: [],
  },
  create(context) {
    return {
      ...scoped.create(context),
      // `open ? <ChevronDown /> : <ChevronRight />`, in braces or in a prop.
      ConditionalExpression(node) {
        if (chevron(node.consequent) && chevron(node.alternate))
          context.report({ node, messageId: "swap" });
      },
    };
  },
};

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "disclosure-family": disclosureFamily,
};
