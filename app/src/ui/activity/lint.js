// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The activity family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/activity-family` (error: the family has moved, #2917): an activity item's old
 * classes, and `data-scope="activity"` or `"activity-bar"` written by hand.
 */

import { moved } from "../familyLint.js";

const activityFamily = moved({
  description: "A view's icon on a side's bar is the shared activity family",
  scopes: ["activity", "activity-bar"],
  classes: ["activity-tab", "activity-count", "activity-said", "activity-divider"],
  message:
    "an activity item or bar: draw it as `<ActivityItem>`, its count as `<ActivityCount>`, its group as `<ActivityGroup>` and its bar as `<ActivityBarFrame>`, from `ui/activity/ActivityItem` (#2917).",
});

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "activity-family": activityFamily,
};
