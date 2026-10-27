// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The titlebar family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/titlebar-family` (error: the title bar's workspaces are a family, #2916): the
 * workspace strip's old classes (`workspaces`, `workspaces-strip`, `workspace`, `plane-root`,
 * `workspace-name`, `workspace-mark`) and `data-scope="titlebar"` by hand.
 */

import { moved } from "../familyLint.js";

const titlebarFamily = moved({
  description: "The title bar's workspaces are the shared title bar family",
  scopes: ["titlebar"],
  classes: [
    "workspaces",
    "workspaces-strip",
    "workspace",
    "plane-root",
    "workspace-name",
    "workspace-mark",
  ],
  message:
    "the title bar's workspaces: draw them as `<WorkspaceChip>` and `<PinnedStrip>` from `ui/titlebar/TitleBarWorkspaces`, and a switcher's parts from `ui/titlebar/Switcher` (#2916).",
});

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "titlebar-family": titlebarFamily,
};
