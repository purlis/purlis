// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The card family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/card-family` (error: the family has moved, #2912): its old classes, and its `data-scope`
 * written by hand, which only its wrapper draws.
 */

import { moved } from "../familyLint.js";

const cardFamily = moved({
  description: "A box one step off its surface is the shared Card family",
  scopes: ["card"],
  classes: ["ui-inline-surface", "ui-inline-surface-*"],
  message: 'a card: draw it as `<Card>` (an ask as `variant="ask"`) from `ui/card/Card` (#2912).',
});

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "card-family": cardFamily,
};
