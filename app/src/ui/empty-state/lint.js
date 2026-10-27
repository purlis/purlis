// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The empty-state family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/empty-state-family` (error: the family has moved, #2913): its old classes, and its `data-scope`
 * written by hand, which only its wrapper draws.
 */

import { moved } from "../familyLint.js";

const emptyStateFamily = moved({
  description: "An empty state is the shared EmptyState family",
  scopes: ["empty-state"],
  classes: ["empty-state", "empty-state-*"],
  message: "an empty state: draw it as `<EmptyState>` from `ui/empty-state/EmptyState` (#2913).",
});

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "empty-state-family": emptyStateFamily,
};
