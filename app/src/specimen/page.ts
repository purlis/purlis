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

/**
 * What the page is handed: the theme, the size it lays itself out at in CSS pixels, the colour of
 * the workspace it is tinted with as the window is there (`null` for none), and the bytes of each
 * face of the theme's fonts, as base64 by the face's digest (#2536): the page may ask purlis for
 * nothing, so its faces are handed to it.
 */
export type Given = {
  theme: Theme;
  width: number;
  height: number;
  tint?: string | null;
  faces?: Record<string, string>;
};

/** The face of digest `asset` that the page was handed, as bytes; `undefined` for one it was
 *  not, which draws the family list's fallback, as the window does for a face purlis would not
 *  serve. */
export function handedFace(handed: Given, asset: string): ArrayBuffer | undefined {
  const text = handed.faces?.[asset];
  if (text === undefined) return undefined;
  const raw = atob(text);
  const bytes = new Uint8Array(raw.length);
  for (let at = 0; at < raw.length; at++) bytes[at] = raw.charCodeAt(at);
  return bytes.buffer;
}

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
