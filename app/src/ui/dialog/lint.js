// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The dialog family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/dialog-family` (error: the dialog family has moved, #2910): a `role="dialog"` or
 * `role="alertdialog"` written by hand. Every dialog and alert dialog is the family's `Dialog`,
 * `AlertDialog` or `DialogSurface` (`ui/dialog/Dialog`), which holds the dialog rules; a popover
 * of another family that is a dialog to a screen reader says so through its own primitive.
 */

import { attributeOf } from "../familyLint.js";

/** The roles only the dialog family writes. */
const DIALOG_ROLES = new Set(["dialog", "alertdialog"]);

const dialogFamily = {
  meta: {
    type: "problem",
    docs: { description: "A dialog is the shared dialog family, never a hand-drawn one" },
    messages: {
      raw: '`role="{{role}}"` by hand: draw it as `<Dialog>`, `<AlertDialog>` or `<DialogSurface>` from `ui/dialog/Dialog`, which keeps the dialog rules (docs/ui-primitives.md).',
    },
    schema: [],
  },
  create(context) {
    return {
      JSXOpeningElement(node) {
        const role = attributeOf(node, "role");
        if (role?.value?.type === "Literal" && DIALOG_ROLES.has(role.value.value))
          context.report({ node: role, messageId: "raw", data: { role: role.value.value } });
      },
    };
  },
};

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "dialog-family": dialogFamily,
};
