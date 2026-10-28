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

/**
 * **This family's controls still drawn as a raw `<button>`** (D-2904-9, #3523): a card pressed
 * whole (the welcome's doors, a repo to get), by the family's one class (`ui-card-control`),
 * never by a word a view picked (`welcome-door`, `repo-card`): the button family's rule lets it
 * through until this family's lane moves them.
 */
export const rawButtons = {
  classes: ["ui-card-control"],
};
