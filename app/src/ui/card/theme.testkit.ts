// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the card family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387). A card's ground (#2912) is read from the family's own
 *  attributes, which its facts and tiles write as they are (#3673): `surface.raised`, which the
 *  button family lists, and in Purlis `surface.base` (its `card.root.fill`), which the engine
 *  holds to the button's grounds. */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  faint: [
    // An unavailable pressed card's words, as a disabled button's (#3655).
    '[data-scope="card"][data-part="pressable"][data-disabled]',
    // Its line, faint with its title, so the description never reads louder than the name.
    '[data-scope="card"][data-part="pressable"][data-disabled] > [data-part="line"]',
  ],
  // Of the family's parts, only its root is a ground a button stands on: a pressed card is
  // itself the button, and holds no other (#3655).
  groundParts: { card: ["root"] },
} satisfies FamilyTheme;
