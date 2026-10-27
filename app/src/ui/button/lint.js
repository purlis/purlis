// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The button family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/button-family` (error: the button family has moved): a raw `<button>` is a control
 * of another family that has not moved yet, said by its role or its class (`otherFamilies`,
 * set in `eslint.config.js`), or it is an error; and no control outside `src/ui` wears the
 * family's old classes (`ui-button`, `ui-button-primary`, `ui-button-row`, …), whose look the
 * family draws from its wrapper's attributes. A screen gives a `Button` a place (`className`
 * for margin, width, order), never a look.
 */

import { classStrings, attributeOf, classWordsOf } from "../familyLint.js";

/** The button family's old styling classes: `ui-button` and every `ui-button-*`. */
const BUTTON_CLASS = /(?:^|\s)ui-button(?:-[\w-]+)?(?=\s|$)/;

const buttonFamily = {
  meta: {
    type: "problem",
    docs: { description: "A button is the shared Button family, never a hand-drawn control" },
    messages: {
      oldClass:
        "`{{word}}` is the button family's old class: draw it as `<Button>`, `<IconButton>`, `<ToggleButton>` or `<ButtonRow>` from `ui/button/Button` with a `variant` and a `size` (docs/ui-primitives.md).",
      raw: "A raw `<button>` is the button family's `<Button>`, `<IconButton>` or `<ToggleButton>` (`ui/button/Button`), unless it is another family's control: say which by its role or its family's class (`rawButtons` in that family's `src/ui/<family>/lint.js`).",
    },
    schema: [
      {
        type: "object",
        properties: {
          otherFamilies: {
            type: "object",
            properties: {
              roles: { type: "array", items: { type: "string" } },
              classes: { type: "array", items: { type: "string" } },
            },
            additionalProperties: false,
          },
        },
        additionalProperties: false,
      },
    ],
  },
  create(context) {
    const { roles = [], classes = [] } = context.options[0]?.otherFamilies ?? {};
    return {
      JSXAttribute(node) {
        if (node.name.name !== "className") return;
        for (const [at, text] of classStrings(node.value)) {
          const found = BUTTON_CLASS.exec(text);
          if (found)
            context.report({ node: at, messageId: "oldClass", data: { word: found[0].trim() } });
        }
      },
      JSXOpeningElement(node) {
        if (node.name.type !== "JSXIdentifier" || node.name.name !== "button") return;
        const role = attributeOf(node, "role")?.value;
        if (role?.type === "Literal" && roles.includes(role.value)) return;
        if (classWordsOf(attributeOf(node, "className")).some((word) => classes.includes(word)))
          return;
        context.report({ node, messageId: "raw" });
      },
    };
  },
};

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "button-family": buttonFamily,
};
