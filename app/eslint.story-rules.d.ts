// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** The types of `eslint.story-rules.js`, for its test (`src/storyLint.test.ts`). */
import type { Rule } from "eslint";

/** The stories' rules, by name, as one plugin. */
export const purlisStories: {
  meta: { name: string };
  rules: Record<string, Rule.RuleModule>;
};
