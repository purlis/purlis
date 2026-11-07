// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the Secret ask's body says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387, #5080). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  // Its words are read on the card the ask is drawn on, or on a part's own fill.
  groundParts: { "secret-ask": [] },
  forced: { drawn: {} },
} satisfies FamilyTheme;
