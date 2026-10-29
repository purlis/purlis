// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the card family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387). A card's ground (#2912) is read from the family's own
 *  attributes, which its facts and tiles write as they are (#3673): `surface.raised`, which the
 *  button family lists, and in Purlis `surface.base` (its `card.root.fill`), which the engine
 *  holds to the button's grounds. */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {} satisfies FamilyTheme;
