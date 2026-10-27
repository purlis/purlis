// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The separator family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/separator-family` (error: the family has moved, #2912): its old classes, and its `data-scope`
 * written by hand, which only its wrapper draws.
 * And a raw `<hr>`, which is the family's `Separator` (`ui/separator/Separator`).
 */

import { moved } from "../familyLint.js";

const separatorFamily = moved({
  description: "A line between two groups is the shared Separator family",
  scopes: ["separator"],
  classes: [],
  elements: ["hr"],
  message: "a separator: draw it as `<Separator>` from `ui/separator/Separator` (#2912).",
});

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "separator-family": separatorFamily,
};
