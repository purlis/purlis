// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **How the window cases words it puts together**: a leaf, so a surface imports it without the
 * module it used to live in (a chat's end, #6488).
 */

/** `words` with its first letter a capital, as a badge or a sentence starts: "Done". */
export function capitalised(words: string): string {
  return words.charAt(0).toUpperCase() + words.slice(1);
}
