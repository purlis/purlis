// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { PANE_FLOOR, type Size } from "./paneDrop";
import type { Direction } from "./tabs";

/**
 * **Open beside, judged before the press** (#4769; prototype `view-open-beside`, accepted on
 * #4705). A pressed view opens in a tab of its own, or, by the beside icon, an ⌥-press, the
 * press's menu or ⌥Enter in the palette, in a split beside a pane: the view's own pane for a
 * button on a view, the note's pane for an offer, the focused pane otherwise.
 *
 * - **The split follows the pane's shape**: a wider pane splits side by side, a taller one
 *   stacked (#4583).
 * - **A split that would leave a side under the floor is refused before the press** (#4705 Q2
 *   A): the floor is #4583's one minimum, `PANE_FLOOR`, which drops, Split right and the
 *   separator use too. The icon and the menu row grey out with the reason as their label, and
 *   an ⌥-press says it and offers Open in a tab. Nothing moves.
 */

/** Whether a view can open beside a pane, and how its split would go. */
export type Beside =
  { ok: true; direction: Direction } | { ok: false; direction: Direction; why: string };

/** **Open beside, for a pane of `size`**; a pane not measured yet splits side by side. */
export function besideOf(size: Size | undefined): Beside {
  if (size === undefined) return { ok: true, direction: "row" };
  const direction: Direction = size.width > size.height ? "row" : "column";
  const across = direction === "row";
  const fits = across ? size.width / 2 >= PANE_FLOOR.width : size.height / 2 >= PANE_FLOOR.height;
  if (fits) return { ok: true, direction };
  return {
    ok: false,
    direction,
    why: across
      ? "Too narrow to open beside: each side needs 40 columns"
      : "Too short to open beside: each side needs 8 rows",
  };
}

/** The beside icon's label: where it opens `name`, or why it can't. `where` is the pane the
 *  icon sits on (a button on a view, an offer on a band) or the focused pane (a panel row, the
 *  palette, the Notices centre). */
export function besideLabel(name: string, where: "this" | "focused", beside: Beside): string {
  if (!beside.ok) return beside.why;
  return `Open ${name} beside ${where === "this" ? "this pane" : "the focused pane"} (⌥-press)`;
}

/** What an ⌥-press on a refused Open beside says, beside its Open in a tab. */
export function besideRefusedSaid(beside: Extract<Beside, { ok: false }>): string {
  return `Not opened beside: ${beside.why.replace(/^Too (narrow|short) to open beside: /, "")}.`;
}

/** What is announced once `name` opened beside `there`. */
export function openedBesideSaid(name: string, there: string, direction: Direction): string {
  return `Opened ${name} ${direction === "row" ? "right of" : "below"} ${there}. Focus is in it.`;
}

/** **What a view row's two keys do** (#4769): Enter opens it in a tab, ⌥Enter beside the
 *  focused pane, or says why it can't before it is pressed. */
export function besideHint(beside: Beside, mac: boolean): string {
  const enter = mac ? "↩" : "Enter";
  const alt = mac ? "⌥↩" : "Alt+Enter";
  return beside.ok
    ? `${enter} Open in a tab · ${alt} Open beside the focused pane`
    : `${enter} Open in a tab · ${alt} ${beside.why}`;
}
