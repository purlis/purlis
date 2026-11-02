// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the steps family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387, #4480). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  forced: {
    drawn: {
      '[data-scope="steps"][data-part="mark"]': { border: "1px solid CanvasText" },
      '[data-scope="steps"][data-part="item"][data-here] [data-part="press"]': {
        outline: "2px solid Highlight",
        "outline-offset": "-2px",
      },
    },
  },
} satisfies FamilyTheme;
