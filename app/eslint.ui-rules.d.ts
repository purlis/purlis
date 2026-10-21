// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** The types of `eslint.ui-rules.js`, for its test (`src/ui/lint.test.ts`). */
import type { Rule } from "eslint";

export const purlisUi: {
  meta: { name: string };
  rules: Record<
    "button-family" | "field-family" | "dialog-family" | "tooltip-family",
    Rule.RuleModule
  >;
};
