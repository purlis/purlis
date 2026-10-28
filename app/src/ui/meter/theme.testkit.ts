// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the meter family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387, #3674). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  forced: {
    drawn: {
      '[data-scope="meter"]:is([data-part="track"], [data-part="fill"])': {
        "forced-color-adjust": "none",
      },
      '[data-scope="meter"][data-part="track"]': {
        border: "1px solid CanvasText",
        "background-color": "Canvas",
      },
      '[data-scope="meter"][data-part="fill"]': { "background-color": "Highlight" },
      '[data-scope="meter"][data-part="root"][data-variant="baseline"] [data-part="fill"]': {
        "background-image": "none",
      },
      '[data-scope="meter"][data-part="root"][data-variant="previous"] [data-part="fill"]': {
        "background-color": "CanvasText",
      },
      '[data-scope="meter"][data-part="root"][data-indeterminate] [data-part="fill"]': {
        "background-color": "Highlight",
        "background-image": "none",
      },
    },
  },
} satisfies FamilyTheme;
