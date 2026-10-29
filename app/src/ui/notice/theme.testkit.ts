// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the notice family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  forced: {
    dangerous: ['[data-scope="notice"][data-part="root"][data-variant="trouble"]'],
    // What needs a decision keeps a shape of its own beside its mark (#3678).
    drawn: {
      '[data-scope="notice"][data-part="root"][data-variant="attention"]': {
        "outline-style": "dotted",
        "outline-color": "CanvasText",
      },
    },
  },
} satisfies FamilyTheme;
