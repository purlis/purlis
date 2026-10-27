// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The badge family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/badge-family` (error: the family has moved, #2912): its old classes, and its `data-scope`
 * written by hand, which only its wrapper draws.
 */

import { moved } from "../familyLint.js";

const badgeFamily = moved({
  description: "A badge or a count is the shared badge and count family",
  scopes: ["badge", "count"],
  classes: ["ui-setting-badge", "count-pill", "count-pill-*", "show-more-needs", "activity-said"],
  message:
    "a badge or a count: draw it as `<Badge>`, `<Count>` or `<CountGroup>` from `ui/badge/Badge` (#2912).",
});

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "badge-family": badgeFamily,
};
