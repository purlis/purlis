// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The menu family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/menu-family` (error: the menu, context menu and popover families have moved, #2909):
 * no element outside `src/ui` says a menu's role (`menu`, `menuitem`, …) or wears the old menu
 * and popover classes (`ui-menu`, `ui-menu-*`, `ui-popover`), and nothing imports Radix's
 * menus or popover. A menu is the family's `Menu`, `ContextMenu` or `InlineMenu`
 * (`ui/menu/Menu.tsx`), a popover one of `ui/popover/Popover.tsx`'s.
 */

import { classStrings, attributeOf } from "../familyLint.js";

/** The roles a menu's elements say, which only the menu family writes. */
const MENU_ROLES = new Set(["menu", "menubar", "menuitem", "menuitemcheckbox", "menuitemradio"]);

/** The old menu and popover classes: `ui-menu`, every `ui-menu-*`, and `ui-popover`. */
const MENU_CLASS = /(?:^|\s)ui-(?:menu(?:-[\w-]+)?|popover)(?=\s|$)/;

/** Radix's menus and popover, and the window's old wrappers of them. */
const MENU_IMPORT =
  /^@radix-ui\/react-(?:dropdown-menu|context-menu|menubar|popover|hover-card)$|(?:^|\/)overlays\/(?:dropdown-menu|context-menu|popover)$/;

/** The words of an answer that grants, anywhere in a label ("Always allow"): no menu row says
 *  them. Shared with the test that holds every menu's words at run time (`menuModel.test.ts`). */
export const GRANT_WORDS = /\b(?:allow|accept|trust|grant)\b/i;

/** A child expression that is words (a name, a field, text), not an element drawn in a row. */
const WORDS = new Set(["Identifier", "MemberExpression", "Literal", "TemplateLiteral"]);

const menuFamily = {
  meta: {
    type: "problem",
    docs: {
      description: "A menu, a context menu or a popover is the shared family, never drawn by hand",
    },
    messages: {
      role: '`role="{{role}}"`: a menu is the menu family\'s `Menu`, `ContextMenu` or `InlineMenu` (`ui/menu/Menu.tsx`), which says its roles itself (docs/ui-primitives.md, "Menus").',
      oldClass:
        "`{{word}}` is an old menu or popover class: the families draw themselves from their parts (`ui/menu/Menu.tsx`, `ui/popover/Popover.tsx`).",
      oldImport:
        "`{{from}}`: a menu or a popover is drawn from its family in `src/ui` (`ui/menu/Menu.tsx`, `ui/popover/Popover.tsx`).",
      noTextValue:
        "A `MenuItem` that draws more than words says them in `textValue`, so typing a letter finds it.",
      grants:
        '`{{said}}` grants something: a grant is a button with `variant="grants"`, never a menu row, so its lock and its edge are drawn (ADR 0097 §6).',
    },
    schema: [],
  },
  create(context) {
    return {
      JSXElement(node) {
        const open = node.openingElement;
        if (open.name.type !== "JSXIdentifier" || open.name.name !== "MenuItem") return;
        const textValue = attributeOf(open, "textValue");
        const words = node.children
          .filter((child) => child.type === "JSXText")
          .map((child) => child.value)
          .join("")
          .trim();
        const onlyWords = node.children.every(
          (child) =>
            child.type === "JSXText" ||
            (child.type === "JSXExpressionContainer" && WORDS.has(child.expression.type)),
        );
        if (textValue === undefined && !onlyWords)
          context.report({ node: open, messageId: "noTextValue" });
        const said =
          textValue?.value?.type === "Literal" && typeof textValue.value.value === "string"
            ? textValue.value.value
            : words;
        if (GRANT_WORDS.test(said))
          context.report({ node: open, messageId: "grants", data: { said } });
      },
      ImportDeclaration(node) {
        if (MENU_IMPORT.test(node.source.value))
          context.report({ node, messageId: "oldImport", data: { from: node.source.value } });
      },
      JSXAttribute(node) {
        if (node.name.name === "role") {
          const role = node.value;
          if (role?.type === "Literal" && MENU_ROLES.has(role.value))
            context.report({ node, messageId: "role", data: { role: role.value } });
          return;
        }
        if (node.name.name !== "className") return;
        for (const [at, text] of classStrings(node.value)) {
          const found = MENU_CLASS.exec(text);
          if (found)
            context.report({ node: at, messageId: "oldClass", data: { word: found[0].trim() } });
        }
      },
    };
  },
};

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "menu-family": menuFamily,
};
