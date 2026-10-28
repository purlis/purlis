// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The meter family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/meter-family` (error, #3674): a raw `<meter>` or `<progress>`, a meter's or a progress
 * bar's role written by hand, and its `data-scope` written by hand, which only its wrapper draws.
 * A value drawn as a bar is the family's `Meter`, `MeterGroup` or `ProgressBar`.
 */

import { moved } from "../familyLint.js";

const meterFamily = moved({
  description: "A value drawn as a bar is the shared meter family",
  scopes: ["meter"],
  classes: [],
  roles: ["meter", "progressbar"],
  elements: ["meter", "progress"],
  message:
    "a value drawn as a bar: draw it as `<Meter>`, `<MeterGroup>` or `<ProgressBar>` from `ui/meter/Meter` (#3674).",
});

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "meter-family": meterFamily,
};

/** The files a rule is still off in, until their lanes move them. */
export const off = {
  // Get project's clone progress (#3674's follow-up): a native bar inside its status line.
  "meter-family": ["src/GetProject.tsx"],
};
