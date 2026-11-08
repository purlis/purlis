// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **How a story's picture is compared with its baseline** (#3178): Vitest's `pixelmatch`
 * comparator, its options written here rather than left to its defaults. Why each has its value,
 * and what it lets through, is `docs/component-stories.md#what-a-picture-lets-through`;
 * `pictures.test.ts` holds the changes it names. The same options decide when a picture is
 * stable enough to compare: two shots in a row no further apart than this.
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
