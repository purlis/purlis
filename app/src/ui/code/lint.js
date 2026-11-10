// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The code family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/code-family` (error, #4481): its `data-scope` written by hand, which only its wrapper
 * writes. Text to copy is the family's `CodeBlock`.
 */

import { moved } from "../familyLint.js";

const codeFamily = moved({
  description: "Text to copy is the shared code family",
  scopes: ["code"],
  classes: [],
  roles: [],
  elements: [],
  message: "text to copy: draw it as `<CodeBlock>` from `ui/code/CodeBlock`",
});

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "code-family": codeFamily,
};
