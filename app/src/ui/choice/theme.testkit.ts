// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the choice family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  faint: [
    // A choice's marks (#2907): the edge of a box and of a radio's circle at rest, and every
    // choice's words and marks while it cannot be used.
    '[data-scope="checkbox"][data-part="box"]',
    '[data-scope="radio"][data-part="mark"]',
    '[data-scope="checkbox"][data-part="root"][data-disabled]',
    '[data-scope="radio"][data-part="root"][data-disabled]',
    '[data-scope="switch"][data-part="root"][data-disabled]',
    '[data-scope="checkbox"][data-part="root"][data-variant="chip"][data-disabled]',
    '[data-scope="checkbox"][data-part="root"][data-disabled][data-selected] [data-scope="checkbox"][data-part="box"]',
    '[data-scope="checkbox"][data-part="root"][data-disabled][data-indeterminate] [data-scope="checkbox"][data-part="box"]',
    '[data-scope="radio"][data-part="root"][data-disabled][data-selected] [data-scope="radio"][data-part="mark"]',
    '[data-scope="switch"][data-part="root"][data-disabled] [data-scope="switch"][data-part="thumb"]',
    '[data-scope="switch"][data-part="root"][data-disabled][data-selected] [data-scope="switch"][data-part="track"]',
    '[data-scope="segment"][data-part="root"][data-disabled]',
    // The chosen segment's 1px edge under it, a mark (R39).
    '[data-scope="segment"][data-part="root"][data-selected]',
    '[data-scope="slider"][data-part="root"][data-disabled]',
    '[data-scope="slider"][data-part="root"][data-disabled] [data-scope="slider"][data-part="fill"]',
    '[data-scope="slider"][data-part="root"][data-disabled] [data-scope="slider"][data-part="thumb"]',
    '[data-scope="option-card"][data-part="root"][data-disabled]',
    '[data-scope="option-card"][data-part="root"][data-variant="tile"][data-disabled]',
  ],
  quiet: [
    // A choice's line under its words, a card's detail and a segment at rest (#2907).
    '[data-scope="checkbox"][data-part="description"]',
    '[data-scope="radio"][data-part="description"]',
    '[data-scope="option-card"][data-part="detail"]',
    '[data-scope="segment"][data-part="root"]',
  ],
  forced: {
    selected: ['[data-scope="segment"][data-part="root"][data-selected]'],
    underlined: ['[data-scope="segment"][data-part="root"][data-selected]'],
    drawn: {
      // A switch's thumb, on the side it is switched to, and a radio's dot keep their shape.
      '[data-scope="switch"][data-part="thumb"]': { "background-color": "CanvasText" },
      '[data-scope="switch"][data-part="root"][data-selected] [data-scope="switch"][data-part="thumb"]':
        { "background-color": "HighlightText" },
      '[data-scope="radio"][data-part="mark"]::after': { "background-color": "CanvasText" },
    },
  },
} satisfies FamilyTheme;
