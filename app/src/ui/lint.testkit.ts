// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
/**
 * Test-only: **a family's lint, run on code a screen would really write** (#2904, #3387). Each
 * family's `lint.test.ts`, beside its `lint.js`, runs its rules through {@link tester}, found by
 * name in {@link rules} as `eslint.ui-rules.js` gathers them.
 */
import { RuleTester } from "eslint";
import tseslint from "typescript-eslint";
import { describe, it } from "vitest";
import { purlisUi } from "../../eslint.ui-rules.js";

RuleTester.describe = describe;
RuleTester.it = it;
RuleTester.itOnly = it.only;

/** A rule tester that reads TSX, as the window's own lint does. */
export const tester = new RuleTester({
  languageOptions: {
    parser: tseslint.parser,
    parserOptions: { ecmaFeatures: { jsx: true } },
  },
});

/** Every family's rules, by name (`button-family` for `purlis/button-family`). */
export const rules = purlisUi.rules;
