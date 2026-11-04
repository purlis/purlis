// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the pane foot says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387, #5202). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  // No part of the foot is a ground a button stands on: its root has no fill, and the track and
  // the fill on it are a bar under `ctx`'s number, with nothing drawn on them.
  groundParts: { "pane-foot": [] },
  forced: {
    drawn: {
      '[data-scope="pane-foot"]:is([data-part="track"], [data-part="fill"])': {
        "forced-color-adjust": "none",
      },
      '[data-scope="pane-foot"][data-part="track"]': {
        border: "1px solid CanvasText",
        "background-color": "Canvas",
      },
      '[data-scope="pane-foot"][data-part="fill"]': { "background-color": "Highlight" },
    },
  },
} satisfies FamilyTheme;
