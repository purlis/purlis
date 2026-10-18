// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * What the theme specimen's page is handed, and how it says it is drawn: the two halves of its
 * contract with `specimen.rs`, which holds the words below equal to its own.
 */
import type { Theme } from "../theme/theme";

/** The global the page is handed what to draw on, before any of its code runs. */
export const GIVEN = "__PURLIS_SPECIMEN__";

/** The title the page sets once it is drawn and may be photographed. */
export const READY = "purlis-specimen-drawn";

/** The title the page sets when it could not draw itself, before why. */
export const FAILED = "purlis-specimen-failed: ";

/** What the page is handed: the theme, and the size it lays itself out at, in CSS pixels. */
export type Given = { theme: Theme; width: number; height: number };

export function given(): Given {
  const handed = (globalThis as Record<string, unknown>)[GIVEN] as Partial<Given> | undefined;
  if (!handed?.theme || !handed.width || !handed.height)
    throw new Error("the specimen page was opened without what it draws");
  return handed as Given;
}

/** Says the page is drawn. */
export function drawn(): void {
  document.title = READY;
}

/** Says the page could not draw itself, and why. */
export function failed(why: unknown): void {
  document.title = `${FAILED}${why instanceof Error ? why.message : String(why)}`;
}
