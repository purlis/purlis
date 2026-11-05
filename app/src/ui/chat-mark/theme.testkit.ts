// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the chat mark family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  classless: [
    // Its parts are spans the wrapper draws, each by its own attributes.
    '[data-scope="chat-mark"][data-part="root"]',
    '[data-scope="chat-mark"][data-part="who"]',
    '[data-scope="chat-mark"][data-part="badge"]',
  ],
} satisfies FamilyTheme;
