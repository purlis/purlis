// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **When a chat ended, as a clock says it** (#6214, #6217): what a closed chat's card and a
 * pane's `chat 641` hover both say. A module of its own and of nothing else, so the link
 * surfaces a screen with no core draws (the theme specimen's) can say it too.
 */

/** When it ended, as a clock says it: `11:08`. Nothing where the record keeps no end. */
export function endedAt(ended: string | null): string | undefined {
  if (ended === null) return undefined;
  const at = new Date(ended);
  if (Number.isNaN(at.getTime())) return undefined;
  return at.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false });
}
