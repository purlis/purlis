// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the message family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387, #5649). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  quiet: [
    // A row's time and its meta (#5649): when it happened, what it is and where.
    '[data-scope="message"][data-part="time"]',
    '[data-scope="message"][data-part="meta"]',
  ],
  forced: {
    drawn: {
      '[data-scope="message"][data-part="mark"]': { border: "1px solid CanvasText" },
    },
  },
} satisfies FamilyTheme;
