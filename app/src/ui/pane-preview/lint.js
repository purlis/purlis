// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The pane preview's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/pane-preview-family` (error, #5291): its `data-scope` written by hand, which only its
 * wrapper writes. What a drag draws over the panes is the family's.
 */

import { moved } from "../familyLint.js";

const panePreviewFamily = moved({
  description: "What a drag draws over the panes is the shared pane preview family",
  scopes: ["pane-preview"],
  classes: [],
  roles: [],
  elements: [],
  message: "what a drag draws over the panes: draw it from `ui/pane-preview/PanePreview`",
});

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "pane-preview-family": panePreviewFamily,
};

/** The files a rule is still off in, until their lanes move them. */
export const off = {};
