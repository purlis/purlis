// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The rail family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/rail-family` (error: the project rail is a family, #2915): the project strip's old
 * classes (`projects`, `project`, `project-name`) and `data-scope="rail"` by hand.
 */

import { moved } from "../familyLint.js";

const railFamily = moved({
  description: "The project rail is the shared rail family",
  scopes: ["rail"],
  classes: ["projects", "project", "project-name"],
  message:
    "the project rail: draw it as `<Rail>`, `<RailProject>` and `<RailAction>` from `ui/rail/Rail` (#2915).",
});

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "rail-family": railFamily,
};
