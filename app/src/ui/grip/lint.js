// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The grip family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/grip-family` (error: the family has moved, #5290): the grip's `data-scope` written by
 * hand, which only its wrapper draws, and the old grip's and Place…'s classes.
 */

import { moved } from "../familyLint.js";

const gripFamily = moved({
  description: "What a pane or a chat is picked up by is the shared Grip family",
  scopes: ["grip"],
  classes: ["chat-row-place"],
  message: "a grip: draw it as `<Grip>` from `ui/grip/Grip` (#5290).",
});

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "grip-family": gripFamily,
};
