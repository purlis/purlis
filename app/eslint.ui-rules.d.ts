// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** The types of `eslint.ui-rules.js`, for the families' lint tests (`src/ui/<family>/lint.test.ts`). */
import type { Linter, Rule } from "eslint";

/** The words of an answer that grants, which no menu row says. */
export const GRANT_WORDS: RegExp;

/** Every family with a lint of its own (`src/ui/<family>/lint.js`), by name. */
export const LINTED_FAMILIES: readonly string[];

/** Every family's rules, by name, as one plugin. */
export const purlisUi: {
  meta: { name: string };
  rules: Record<string, Rule.RuleModule>;
};

/** What a raw `<button>` may say it is another family's control by. */
export interface RawButtons {
  roles: string[];
  classes: string[];
}

/** Every family's `rawButtons`, with `more`. */
export function rawButtons(more?: RawButtons): RawButtons;

/** The files each family's rule is still off in. */
export const stillOff: Linter.Config[];
