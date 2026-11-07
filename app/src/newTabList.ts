// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Does, Offer } from "./actions";

/**
 * **The strip's `+` list** (#5292; spec #5288 stories 73 to 81): what can be opened in a new tab,
 * drawn from the action catalogue. A row is any offer the catalogue marks `plus` where it builds
 * it, so a view the catalogue starts offering joins the list with no menu of its own, and one it
 * stops offering leaves it. Nothing here names a row.
 *
 * What starts comes first (Empty tab, New chat, New shell), then the views, each group in the
 * catalogue's order. **Every row means a new tab** (the call on #5292 from #5297): a new chat or
 * shell chosen here gets a tab of its own, even where the front tab has an empty pane. The
 * sidebar's New chat and ⌘N keep the one rule, which fills an empty pane first.
 */

/** One row of the list: the catalogue's offer, the words the list says, and its group. */
export type NewTabRow = { offer: Offer; said: string; group: "start" | "view" };

/** **Past this many rows, the list scrolls and has a filter** (spec #5288 story 80; "about 8"). */
export const FILTER_FROM = 8;

/** How long the pointer rests on `+` before its list opens, and how long the list waits for the
 *  pointer to come back once it has left both (#5292, the prototype's starting values). */
export const OPENS_AFTER_MS = 160;
export const CLOSES_AFTER_MS = 260;

/** The list's rows, from the catalogue: what starts, then the views. */
export function newTabRows(offers: readonly Offer[]): NewTabRow[] {
  const rows = offers.flatMap((offer): NewTabRow[] =>
    offer.plus === undefined
      ? []
      : [{ offer, said: offer.plus.said ?? offer.title, group: offer.plus.group }],
  );
  return [
    ...rows.filter((row) => row.group === "start"),
    ...rows.filter((row) => row.group === "view"),
  ];
}

/** The rows whose words hold `typed`, whatever its case. All of them for nothing typed. */
export function shown(rows: readonly NewTabRow[], typed: string): NewTabRow[] {
  const wanted = typed.trim().toLocaleLowerCase();
  if (wanted === "") return [...rows];
  return rows.filter((row) => row.said.toLocaleLowerCase().includes(wanted));
}

/**
 * **What a row chosen here does**: a new chat or a new shell in a tab of its own. Every other row
 * already opens a tab of its own (an empty tab, a view's tab), so its offer is the catalogue's.
 */
export function inANewTab(offer: Offer): Offer {
  const does: Does = offer.does;
  if (does.verb === "chat.new" || does.verb === "newShell")
    return { ...offer, does: { ...does, ownTab: true } };
  return offer;
}
