// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the list family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  faint: [
    // A list's row that cannot be used (#2911).
    '[data-scope="list"][data-part="root"][data-disabled]',
    '[data-scope="list"][data-part="root"][data-unavailable]',
  ],
  quiet: [
    // A list's group name (#2911): a day, a file.
    '[data-scope="list"][data-part="header"]',
  ],
  classless: [
    // A fold's row under the pointer (#3670): a group's name, its count and its twist, or a
    // run's quiet line; never a button. A run's closing line, which holds one, takes no pill.
    ':is(\n  [data-scope="list"][data-part="header"][data-fold],\n  [data-scope="list"][data-part="run"]:not([data-end])\n)[data-hovered]',
  ],
  forced: {
    selected: ['[data-scope="list"][data-part="root"][data-selected]'],
    // A chosen row keeps its edge (#2911).
    drawn: {
      '[data-scope="list"][data-part="root"][data-selected]': {
        "box-shadow": "inset 2px 0 0 SelectedItemText",
      },
    },
  },
} satisfies FamilyTheme;
