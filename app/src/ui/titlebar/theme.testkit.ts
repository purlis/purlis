// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the titlebar family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  classless: [
    // A warning's notice dot (#3649), its part named by its kind (`KINDS` in
    // `TitleBarWorkspaces.tsx`): a 16px mark over a chip's corner, which no button stands on.
    '[data-part="dot-warning"][data-scope="titlebar"]',
  ],
  faint: [
    // The slash between the title bar's project and its pinned workspaces, a mark (#2916, R156).
    '[data-part="slash"][data-scope="titlebar"]',
    // A switcher row's pin, a mark (R107, R156), and a row that cannot run, the disabled tier.
    '[data-part="pin"][data-scope="titlebar"]',
    '[data-part="row"][data-scope="titlebar"][data-unavailable]',
  ],
} satisfies FamilyTheme;
