// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The tooltip family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/tooltip-family` (error: the tooltip family has moved, #2908): a native `title=` on an
 * element, a primitive's part (`RadioGroup.Item`), a member of the button or status item
 * family, or a house component that hands it on to its element (`handsOn`, set in
 * `eslint.config.js`). A hint is the shared tooltip: `<Tooltip tip>` from `ui/tooltip/Tooltip`
 * around any element, `tooltip` on a `Button`, a `ToggleButton`, a `StatusItem` or a
 * `StatusCell`, and an `IconButton`'s name, which its tooltip shows. A frame's `title` is its
 * name, not a tooltip, and stays.
 */

import { attributeOf } from "../familyLint.js";

/** The members of a family whose hint is their `tooltip`, never a `title`: the button family's,
 *  and the status item family's. */
const BUTTONS = new Set(["Button", "IconButton", "ToggleButton", "StatusItem", "StatusCell"]);

/** The elements whose `title` is their accessible name, which no tooltip replaces. */
const TITLED = new Set(["iframe", "frame"]);

/** The `title` key an object written as a literal holds, if it holds one. */
const titleKeyOf = (object) =>
  object?.type === "ObjectExpression"
    ? object.properties.find(
        (one) =>
          one.type === "Property" &&
          ((one.key.type === "Identifier" && one.key.name === "title") ||
            (one.key.type === "Literal" && one.key.value === "title")),
      )
    : undefined;

/** The object literal a spread hands on: written in place, or a `const` of this scope. */
function spreadObject(context, spread) {
  const argument = spread.argument;
  if (argument.type === "ObjectExpression") return argument;
  if (argument.type !== "Identifier") return undefined;
  let scope = context.sourceCode.getScope(spread);
  while (scope) {
    const variable = scope.set.get(argument.name);
    if (variable) {
      const definition = variable.defs[0];
      return definition?.type === "Variable" ? definition.node.init : undefined;
    }
    scope = scope.upper;
  }
  return undefined;
}

const tooltipFamily = {
  meta: {
    type: "problem",
    docs: { description: "A hint is the shared tooltip, never a native title" },
    messages: {
      title:
        "A native `title=` tooltip: draw the hint with the shared tooltip, `<Tooltip tip>` from `ui/tooltip/Tooltip`, or a button's `tooltip`; an `IconButton`'s tooltip is its `aria-label` (docs/ui-primitives.md).",
      svgTitle:
        "An SVG `<title>` is a native tooltip: draw the hint with the shared tooltip around the drawing (`<Tooltip tip>`), and name the drawing with `aria-label` where it needs a name.",
    },
    schema: [
      {
        type: "object",
        properties: {
          // The house components that hand a `title` on to their element as it is.
          handsOn: { type: "array", items: { type: "string" } },
        },
        additionalProperties: false,
      },
    ],
  },
  create(context) {
    const { handsOn = [] } = context.options[0] ?? {};
    /** Whether `node`'s element hands a `title` on to the DOM as a native tooltip. */
    const handsTitleOn = (node) => {
      if (node.name.type === "JSXMemberExpression") return true;
      if (node.name.type !== "JSXIdentifier") return false;
      const tag = node.name.name;
      if (TITLED.has(tag)) return false;
      return /^[a-z]/.test(tag) || BUTTONS.has(tag) || handsOn.includes(tag);
    };
    return {
      JSXOpeningElement(node) {
        if (node.name.type === "JSXIdentifier" && node.name.name === "title") {
          context.report({ node, messageId: "svgTitle" });
          return;
        }
        if (!handsTitleOn(node)) return;
        const title = attributeOf(node, "title");
        if (title) context.report({ node: title, messageId: "title" });
        // A `title` handed on through a spread of an object written here (`{...shared}`).
        for (const spread of node.attributes.filter((one) => one.type === "JSXSpreadAttribute")) {
          const key = titleKeyOf(spreadObject(context, spread));
          if (key) context.report({ node: key, messageId: "title" });
        }
      },
    };
  },
};

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "tooltip-family": tooltipFamily,
};
