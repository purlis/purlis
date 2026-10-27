// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the panel family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  faint: [
    // A resize handle's grip under the pointer, a mark (`ui/panel/panel.css`, #3143).
    '[data-scope="panel-separator"][data-part="root"][data-separator="hover"] > [data-scope="panel-separator"][data-part="grip"]',
    '[data-scope="panel-separator"][data-part="root"][data-separator="focus"]:hover:not(:focus-visible) > [data-scope="panel-separator"][data-part="grip"]',
  ],
  spreadGrounds: [
    // The content panel the tabs attach to (`panelRoot("content")`), Purlis's panel fill.
    ["panes", ["surface.raised"]],
  ],
} satisfies FamilyTheme;
