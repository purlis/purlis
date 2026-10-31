// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the status family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  classless: [
    // The Notices item's warning dot (#3653), its part named by its kind (`KINDS` in
    // `TitleBarWorkspaces.tsx`): a mark over the item's corner, which no button stands on.
    '[data-part="dot-warning"][data-scope="status-item"]',
  ],
  faint: [
    // A disabled status item's words (#2913).
    '[data-scope="status-item"][data-part="root"][data-disabled]',
  ],
  quiet: [
    // A status cell's word before its count (#2913).
    '[data-static]:has(> [data-part="count"]) > [data-scope="status-item"][data-part="label"]',
  ],
  forced: {
    marked: ['[data-scope="status-item"][data-variant="waiting"] > [data-part="count"]'],
    dangerous: ['[data-scope="status-item"][data-part="root"][data-tone="danger"]'],
  },
} satisfies FamilyTheme;
