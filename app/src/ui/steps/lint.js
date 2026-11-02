// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The steps family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/steps-family` (error, #4480): its `data-scope` written by hand, which only its wrapper
 * writes. A guide's list of steps is the family's `StepList`.
 */

import { moved } from "../familyLint.js";

const stepsFamily = moved({
  description: "A guide's list of steps is the shared steps family",
  scopes: ["steps"],
  classes: [],
  roles: [],
  elements: [],
  message: "a guide's list of steps: draw it as `<StepList>` from `ui/steps/Steps`",
});

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "steps-family": stepsFamily,
};

/** The files a rule is still off in, until their lanes move them. */
export const off = {};
