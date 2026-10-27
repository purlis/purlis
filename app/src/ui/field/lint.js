// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The field family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/field-family` (error: the field family has moved, #2905): a raw `<textarea>`, or a
 * raw `<input>` a person types in (no `type`, or `text`, `search`, `password`, `url`, `email`,
 * `tel`, `number`, or a `type` decided at run time), is the family's `TextField` or `TextArea`
 * (`ui/field/Field`); and no control wears the family's old classes (`ui-input`,
 * `ui-input-lead`, `ui-list-field`, …).
 */

import { classStrings, attributeOf, CHOICES, typeOf } from "../familyLint.js";

/** The field family's old styling classes: `ui-input`, every `ui-input-*`, `ui-list-field`, and
 *  the old field wrapper, `ui-field` and every `ui-field-*` (#3129), whose box the family draws. */
const FIELD_CLASS = /(?:^|\s)(ui-input(?:-[\w-]+)?|ui-list-field|ui-field(?:-[\w-]+)?)(?=\s|$)/;

const fieldFamily = {
  meta: {
    type: "problem",
    docs: { description: "A box a person types in is the shared field family" },
    messages: {
      raw: "A raw `<{{tag}}>`: draw it as `<TextField>` or `<TextArea>` from `ui/field/Field`, with its label, help and validation (docs/ui-primitives.md).",
      oldClass:
        "`{{word}}` is the field family's old class: draw the box as `<TextField>` or `<TextArea>` from `ui/field/Field` (docs/ui-primitives.md).",
    },
    schema: [],
  },
  create(context) {
    return {
      JSXAttribute(node) {
        if (node.name.name !== "className") return;
        for (const [at, text] of classStrings(node.value)) {
          const found = FIELD_CLASS.exec(text);
          if (found) context.report({ node: at, messageId: "oldClass", data: { word: found[1] } });
        }
      },
      JSXOpeningElement(node) {
        if (node.name.type !== "JSXIdentifier") return;
        const tag = node.name.name;
        if (tag === "textarea") context.report({ node, messageId: "raw", data: { tag } });
        if (tag !== "input") return;
        const type = typeOf(node);
        if (type === "hidden" || (type !== null && CHOICES.has(type))) return;
        if (combobox(node)) return;
        context.report({ node, messageId: "raw", data: { tag } });
      },
    };
  },
};

/** Whether an `<input>` is a combobox's, by its role. */
function combobox(node) {
  const role = attributeOf(node, "role")?.value;
  return role?.type === "Literal" && role.value === "combobox";
}

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "field-family": fieldFamily,
};
