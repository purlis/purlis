// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The checks family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/checks-family` (error, #4481): its `data-scope` written by hand, which only its wrapper
 * writes. A list of things checked is the family's `CheckList`.
 */

import { moved } from "../familyLint.js";

const checksFamily = moved({
  description: "A list of things checked is the shared checks family",
  scopes: ["checks"],
  classes: [],
  roles: [],
  elements: [],
  message: "a list of things checked: draw it as `<CheckList>` from `ui/checks/Checks`",
});

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "checks-family": checksFamily,
};

/** The files a rule is still off in, until their lanes move them. */
export const off = {};
