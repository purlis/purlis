// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** The types of `eslint.ui-rules.js`, for its test (`src/ui/lint.test.ts`). */
import type { Rule } from "eslint";

/** The words of an answer that grants, which no menu row says. */
export const GRANT_WORDS: RegExp;

export const purlisUi: {
  meta: { name: string };
  rules: Record<
    | "button-family"
    | "menu-family"
    | "field-family"
    | "select-family"
    | "choice-family"
    | "dialog-family"
    | "tooltip-family"
    | "panel-family"
    | "panel-section-family"
    | "empty-state-family"
    | "status-item-family"
    | "list-family"
    | "rail-family"
    | "activity-family"
    | "tabs-family"
    | "titlebar-family"
    | "card-family"
    | "notice-family"
    | "badge-family"
    | "separator-family",
    Rule.RuleModule
  >;
};
