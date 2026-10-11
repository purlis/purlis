// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import {
  CATALOGUE,
  openView,
  regionOf,
  shownIn,
  slotOf,
  type Arrangement,
  type Placement,
  type Side,
  type ViewId,
} from "./regions";
import { DEFAULT_TEXT, useTextSizes } from "./textSize";

/**
 * **The Panel in a narrow window** (spec #1736, P-7, #1741).
 *
 * Below about 900 px of window width (at the default text size, {@link isNarrow}) two halves
 * side by side would each be too narrow to read, so the Panel draws one at a time: the one used
 * last. The other side's Lower icons still switch it, so both halves stay a press away.
 *
 * **What is drawn, never what is kept.** {@link narrowed} answers the arrangement to *draw*; the
 * arrangement that is stored is left as the person left it, so a window widened again draws both
 * halves as they were, and nothing about a narrow window is written to `layout.json`. Which half
 * was used last is this window's to know and is not kept either: it is a fact about the last few
 * presses, not about how the person arranged the project.
 */

/** The window width, in CSS pixels at the default window text size, below which the Panel
 *  draws one half at a time. */
export const NARROW_BELOW = 900;

/**
 * Whether a window `width` CSS pixels wide, its text at `text` px, draws one half of the Panel
 * at a time.
 *
 * **Counted at the default text size** ({@link DEFAULT_TEXT}): every size in the window is in
 * `rem`, so the window text size is purlis's zoom, and a bigger text crowds the halves as a
 * narrower window would. It is also what makes the rule reachable at all: the window is never
 * under 1024 px (ADR 0054), so at the default text no window is narrow, and from 16 px of window
 * text the narrowest one is.
 */
export const isNarrow = (width: number, text: number = DEFAULT_TEXT.window): boolean =>
  (width * DEFAULT_TEXT.window) / text < NARROW_BELOW;

/** The half of the Panel on `side`, as `arrangement` places it. */
const halfOn = (arrangement: Arrangement, side: Side): Placement | undefined =>
  arrangement.find((one) => slotOf(one) === "panel" && one.side === side);

const drawn = (one: Placement | undefined): boolean =>
  one !== undefined && shownIn([one]).length > 0;

/**
 * **What a narrow window draws**: `arrangement` with only one half of the Panel out. While both
 * are out, the one on `last`'s side is drawn and the other is drawn as away; with one away, the
 * other is drawn, whole, as it is in a wide window (P-5). The sidebars are not the Panel's and
 * are left as they are.
 */
export function narrowed(arrangement: Arrangement, last: Side): Arrangement {
  const other: Side = last === "left" ? "right" : "left";
  if (!drawn(halfOn(arrangement, last)) || !drawn(halfOn(arrangement, other))) return arrangement;
  return arrangement.map((one) =>
    slotOf(one) === "panel" && one.side === other ? { ...one, collapsed: true } : one,
  );
}

/**
 * **The half used last**, after the arrangement went from `before` to `after`: a half that was
 * just brought back, or switched to another of its views — by its Lower icon, a view's key, a
 * palette row — is the one used. A change that brings both back, as ⌘J does, or touches no half,
 * leaves it where it was; putting a half away is not using it.
 */
export function lastUsed(before: Arrangement, after: Arrangement, last: Side): Side {
  const used = (["left", "right"] as const).filter((side) => {
    const was = halfOn(before, side);
    const now = halfOn(after, side);
    if (now === undefined || !drawn(now)) return false;
    return !drawn(was) || openView(was as Placement) !== openView(now);
  });
  return used.length === 1 ? used[0] : last;
}

/**
 * **A Lower icon pressed in a narrow window**: which half is drawn after it, and whether the
 * press goes on to the window's own pick (`regions.picked`).
 *
 * - **The half not drawn**: it is drawn now. Its open view's icon puts nothing away — the half
 *   was out all along, only not drawn — so the press stops here; another of its views is the
 *   window's pick, which switches to it.
 * - **A half that is away**: the window's pick brings it back, and it is the one drawn.
 * - **The half drawn, or a sidebar's view**: the window's pick, as in a wide window.
 */
export function narrowPicked(
  arrangement: Arrangement,
  view: ViewId,
  last: Side,
  panels: readonly ViewId[] = [],
): { last: Side; forward: boolean } {
  const region = regionOf(view, arrangement);
  const half = arrangement.find((one) => one.id === region);
  if (half === undefined || CATALOGUE[region].half !== true) return { last, forward: true };
  const shown = shownIn(narrowed(arrangement, last).filter((one) => slotOf(one) === "panel"));
  if (shown.some((one) => one.id === region)) return { last, forward: true };
  const outAlready = drawn(half) && openView(half, panels) === view;
  return { last: half.side, forward: !outAlready };
}

/** Whether the window is narrow now, following its width as it is resized and its text size
 *  as it is changed. */
export function useNarrow(): boolean {
  const text = useTextSizes().window;
  const [width, setWidth] = useState(() => window.innerWidth);
  useEffect(() => {
    const measure = () => setWidth(window.innerWidth);
    measure();
    window.addEventListener("resize", measure);
    return () => window.removeEventListener("resize", measure);
  }, []);
  return isNarrow(width, text);
}
