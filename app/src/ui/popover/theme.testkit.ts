// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the popover family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  forced: {
    opened: ['[data-scope="popover"][data-part="root"]'],
    // Its own parts are inside its outline, which says it once.
    drawn: {
      '[data-scope="popover"]:is([data-part="list"], [data-part="body"])': { outline: "none" },
    },
  },
} satisfies FamilyTheme;
