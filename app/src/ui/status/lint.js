// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The status family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/status-item-family` (error: the family has moved, #2913): its old classes, and its `data-scope`
 * written by hand, which only its wrapper draws.
 */

import { moved } from "../familyLint.js";

const statusItemFamily = moved({
  description: "A status bar item or a status pill is the shared status item family",
  scopes: ["status-item", "status-bar"],
  classes: [
    "status-line",
    "status-context",
    "status-health",
    "status-cell",
    "status-label",
    "status-glyph",
    "status-workspace",
    "status-project",
    "status-project-*",
    "status-count",
    "status-unknown",
    "status-badge",
    "status-update",
    "status-pin",
    "status-doctor",
    "status-alerts",
    "doctor-said",
    "doctor-tone-*",
    "needs-you-button",
    "needs-you-number",
    "title-about",
    "title-kill-switch",
  ],
  message:
    "a status item: draw it as `<StatusItem>` or `<StatusCell>` from `ui/status/StatusItem`, on a `<StatusBar>` (`ui/status/StatusBar`) where it is the bar's (#2913).",
});

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "status-item-family": statusItemFamily,
};
