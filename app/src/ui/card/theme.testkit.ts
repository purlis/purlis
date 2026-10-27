// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the card family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  classless: [
    // A card's ground (#2912), set by the family's attributes on the element a screen asks for:
    // `surface.raised`, which the button family lists, and in Purlis `surface.base` (its
    // `card.root.fill`), which the engine holds to the button's grounds.
    '[data-scope="card"][data-part="root"]',
  ],
} satisfies FamilyTheme;
