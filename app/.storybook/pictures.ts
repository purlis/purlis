// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **How a story's picture is compared with its baseline** (#3178, `docs/component-stories.md`):
 * Vitest's `pixelmatch` comparator, its options written here rather than left to its defaults, so
 * what the screenshot gate lets through is a decision with a reason.
 *
 * - **`threshold` 0.015**, on pixelmatch's scale (the YIQ distance of two pixels, 0 to 1; a grey
 *   one level brighter of 255 is about 0.0038 on it). Its default, 0.1, let a whole wash change
 *   hue unseen: Purlis light's `wash.warning` went from a grey-brown to amber in #3169, about
 *   0.061 on this scale, and no picture of the five stories that draw it failed; an accent going
 *   from green to blue at one luminance in #5263 passed too (0.083, and 0.033 on the
 *   `accent.surface` turned toward it), and so would any surface of Purlis light's drawn as its
 *   neighbour (0.018 from the bar to the window, 0.034 to 0.038 for the others). 0.015 fails each
 *   of those and passes two drawings a rounding apart (two levels in a channel, 0.0076 at most),
 *   with room on both sides (`pictures.test.ts` holds each). Lower, it would come within a
 *   rounding of noise; a faint wash turned at one luminance can still pass under it (Purlis
 *   light's `accent.wash`, at .14, moves 0.013 from green to blue).
 * - **No pixel may differ** (`allowedMismatchedPixels` 0, the comparator's own default, written
 *   down). A small control's whole fill is a few hundred pixels, and any allowance big enough to
 *   pass noise would let a small swatch through. What differs between two runs of one tree in the
 *   pinned image is antialiasing at an edge, which the comparator already leaves out
 *   (`includeAA` false, its default), and Chromium's words are greyscale so a glyph draws the same
 *   whatever ran before it (`vitest.project.ts`). A re-run of the stories on an unchanged tree is
 *   the check that it holds.
 */
export const PICTURE_THRESHOLD = 0.015;

/** The pixels a picture may differ from its baseline by, past the threshold. */
export const PICTURE_PIXELS_ALLOWED = 0;

/** What `toMatchScreenshot` is given in every story project (`vitest.project.ts`). */
export const PICTURE_COMPARISON = {
  comparatorName: "pixelmatch",
  comparatorOptions: {
    threshold: PICTURE_THRESHOLD,
    allowedMismatchedPixels: PICTURE_PIXELS_ALLOWED,
  },
} as const;
