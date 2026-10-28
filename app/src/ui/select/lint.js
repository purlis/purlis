// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The select family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/select-family` (error: the select and combobox families have moved, #2906): a raw
 * `<select>` or `<datalist>`, a `role="combobox"` or an `aria-haspopup="listbox"` written by
 * hand, and the old `ui-select` classes. A select is the family's `Select`
 * (`ui/select/Select`), a box with suggestions its `ComboBox`, and a box over a list drawn in
 * place (the palette) its `ListComboBox` (`ui/select/ComboBox`).
 */

import { classStrings, literalOf } from "../familyLint.js";

/** The select family's old styling classes: `ui-select` and `ui-select-list`. */
const SELECT_CLASS = /(?:^|\s)(ui-select(?:-list)?)(?=\s|$)/;

const selectFamily = {
  meta: {
    type: "problem",
    docs: { description: "A select or a combobox is the shared select or combobox family" },
    messages: {
      raw: "A raw `<{{tag}}>`: draw it as `<Select>` from `ui/select/Select`, or a box with suggestions as `<ComboBox>` from `ui/select/ComboBox` (docs/ui-primitives.md).",
      role: "`{{attribute}}` by hand: a combobox is `<ComboBox>` or `<ListComboBox>` from `ui/select/ComboBox`, and a list a button opens is `<Select>` from `ui/select/Select`, which hold its keys and its ARIA (docs/ui-primitives.md).",
      oldClass:
        "`{{word}}` is the select family's old class: draw it as `<Select>` from `ui/select/Select` (docs/ui-primitives.md).",
    },
    schema: [],
  },
  create(context) {
    return {
      JSXAttribute(node) {
        const name = node.name.name;
        if (name === "className") {
          for (const [at, text] of classStrings(node.value)) {
            const found = SELECT_CLASS.exec(text);
            if (found)
              context.report({ node: at, messageId: "oldClass", data: { word: found[1] } });
          }
          return;
        }
        // `role="combobox"`, and `role={"combobox"}` or `` role={`combobox`} ``, alike.
        const value = literalOf(node.value);
        if (
          (name === "role" && value === "combobox") ||
          (name === "aria-haspopup" && value === "listbox")
        )
          context.report({
            node,
            messageId: "role",
            data: { attribute: `${name}="${value}"` },
          });
      },
      JSXOpeningElement(node) {
        if (node.name.type !== "JSXIdentifier") return;
        const tag = node.name.name;
        if (tag === "select" || tag === "datalist")
          context.report({ node, messageId: "raw", data: { tag } });
      },
    };
  },
};

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "select-family": selectFamily,
};
