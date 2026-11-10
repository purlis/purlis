// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The pane foot's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/pane-foot-family` (error, #5202): its `data-scope` written by hand, which only its
 * wrapper writes. The line under a chat's terminal is the family's `PaneFoot`.
 */

import { moved } from "../familyLint.js";

const paneFootFamily = moved({
  description: "The line under a chat's terminal is the shared pane foot family",
  scopes: ["pane-foot"],
  classes: [],
  roles: [],
  elements: [],
  message:
    "the line under a chat's terminal: draw it as `<PaneFoot>`, `<PaneFootSeparator>` or `<PaneFootGauge>` from `ui/pane-foot/PaneFoot`",
});

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "pane-foot-family": paneFootFamily,
};
