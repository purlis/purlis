// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A long list draws only the rows in sight** (#5501, the scale spec #5486): the one wrapper of
 * `@tanstack/react-virtual`, as every family wraps its primitive once (ADR 0104). A list of more
 * than {@link DRAWN_WHOLE} rows draws the rows in its scroller's sight and a few either side,
 * each placed where it stands in the whole, and the list keeps the height of all of them, so the
 * scrollbar is the whole list's. A shorter list is drawn whole, as it always was.
 *
 * What a list a field aims at needs of it (the WAI-ARIA combobox pattern):
 *
 * - **The aimed row is always drawn**, wherever it is, so the field's `aria-activedescendant`
 *   names an element that is there, and the scroller is brought to it as the aim moves.
 * - **Each drawn row says its place in the whole** (`aria-posinset` of `aria-setsize`), so a
 *   screen reader says "152 of 1,002" of a list whose other rows are not in the document.
 *
 * Its geometry is the list's own CSS, read as it is drawn: its padding, its gap between rows, and
 * the height of a row (every row of these lists is one line). A row is taken to be 32 px only
 * until one is measured, and in a document with no layout.
 */
import {
  createContext,
  useContext,
  useLayoutEffect,
  useState,
  type CSSProperties,
  type RefObject,
} from "react";
import {
  defaultRangeExtractor,
  observeElementRect,
  useVirtualizer,
  type Range,
  type Virtualizer,
} from "@tanstack/react-virtual";

/**
 * **The most rows a list draws whole.** A few hundred rows draw well inside a frame; the lists
 * that can reach thousands (a row per workspace, at the spec's 1,000) are drawn in part above it.
 */
export const DRAWN_WHOLE = 250;

/** Rows drawn either side of the ones in sight, so a quick scroll never shows a gap first. */
const OVERSCAN = 8;

/** **Where one drawn row stands**, read by the row of a family that is drawn in part. */
export type RowPlace = {
  /** Its place in the whole, from 0. */
  index: number;
  /** How many rows the whole holds. */
  of: number;
  /** Its top, in px from its list's top. */
  start: number;
};

const Placed = createContext<RowPlace | undefined>(undefined);

/** Gives the row drawn inside it its place. */
export const RowPlaced = Placed.Provider;

/**
 * **What a row spreads on its own element** to stand in a list drawn in part: `aria`, its place
 * said to a reader (an option's only), and `at`, its top. Nothing where the list is drawn whole,
 * so a short list is as it was.
 */
export function useRowPlace(): {
  aria: { "aria-posinset"?: number; "aria-setsize"?: number };
  at: { "data-placed"?: ""; style?: CSSProperties };
} {
  const place = useContext(Placed);
  if (place === undefined) return { aria: {}, at: {} };
  return {
    aria: { "aria-posinset": place.index + 1, "aria-setsize": place.of },
    at: placedAt(place),
  };
}

/** **What an element spreads to stand at `place`**: marked placed, at its top. */
export function placedAt(place: RowPlace): { "data-placed": ""; style: CSSProperties } {
  return { "data-placed": "", style: { insetBlockStart: place.start } };
}

/** **The rows a list draws**, and what its own element carries to keep the whole's height. */
export type DrawnRows = {
  /** The rows to draw, in order, each with its place where the list is drawn in part. */
  rows: readonly { index: number; place?: RowPlace }[];
  /** What the list's element spreads: marked drawn in part, and the whole's height. */
  list: { "data-partly"?: ""; style?: CSSProperties };
};

/** The list's geometry as its CSS draws it, in px. */
type Geometry = { margin: number; padStart: number; padEnd: number; gap: number; row: number };

/** What a list is taken to be before it is first drawn, and in a document with no layout. */
const UNMEASURED: Geometry = { margin: 0, padStart: 0, padEnd: 0, gap: 0, row: 32 };

/**
 * **The rows of a list of `count`**, the list `list` in the scroller `scroller` (the same element
 * where the list scrolls itself). `aimed`, the row its field aims at, is always among them, and
 * the scroller is brought to it as it moves.
 */
export function useDrawnRows({
  count,
  list,
  scroller,
  aimed,
}: {
  count: number;
  list: RefObject<HTMLElement | null>;
  scroller: RefObject<HTMLElement | null>;
  aimed?: number;
}): DrawnRows {
  const partly = count > DRAWN_WHOLE;
  const keep = aimed !== undefined && aimed >= 0 && aimed < count ? aimed : undefined;
  const [geometry, setGeometry] = useState(UNMEASURED);
  // Read once the list is drawn in part, and again once its first row is drawn to be measured.
  useLayoutEffect(() => {
    if (!partly) return;
    const read = measured(list.current, scroller.current);
    if (read !== undefined && !same(read, geometry)) setGeometry(read);
  }, [partly, count, geometry, list, scroller]);
  // The React Compiler leaves this hook unmemoized, as it must: the virtualizer's answers change
  // with every scroll (`react-hooks/incompatible-library`).
  const virtualizer = useVirtualizer({
    count: partly ? count : 0,
    getScrollElement: () => scroller.current,
    estimateSize: () => geometry.row,
    gap: geometry.gap,
    paddingStart: geometry.padStart,
    paddingEnd: geometry.padEnd,
    scrollMargin: geometry.margin,
    overscan: OVERSCAN,
    observeElementRect: laidOrWindow,
    rangeExtractor: (range: Range) => withAimed(defaultRangeExtractor(range), keep),
  });
  useLayoutEffect(() => {
    if (partly && keep !== undefined) virtualizer.scrollToIndex(keep, { align: "auto" });
  }, [partly, keep, virtualizer, geometry]);
  if (!partly) return { list: {}, rows: Array.from({ length: count }, (_, index) => ({ index })) };
  const size = virtualizer.getTotalSize() - geometry.margin - geometry.padStart - geometry.padEnd;
  return {
    list: { "data-partly": "", style: { "--list-rows-size": `${size}px` } as CSSProperties },
    rows: virtualizer.getVirtualItems().map((item) => ({
      index: item.index,
      place: { index: item.index, of: count, start: item.start - geometry.margin },
    })),
  };
}

/**
 * The scroller's size as drawn, or the window's height while it has none: a scroller not laid
 * out yet (or in a document with no layout) would otherwise draw no row at all, not even the
 * aimed one.
 */
function laidOrWindow(
  instance: Virtualizer<HTMLElement, Element>,
  told: (rect: { width: number; height: number }) => void,
) {
  return observeElementRect(instance, (rect) =>
    told(rect.height > 0 ? rect : { width: rect.width, height: window.innerHeight }),
  );
}

/** The geometry of `list` in `scroller` as drawn, or nothing before it is. */
function measured(list: HTMLElement | null, scroller: HTMLElement | null): Geometry | undefined {
  if (list === null || scroller === null) return undefined;
  const style = getComputedStyle(list);
  const px = (value: string) => Number.parseFloat(value) || 0;
  const row = list.querySelector<HTMLElement>("[data-placed]")?.getBoundingClientRect().height;
  return {
    // Where the list's top is in the scroller's content: none where the list is the scroller.
    margin:
      list === scroller
        ? 0
        : list.getBoundingClientRect().top -
          scroller.getBoundingClientRect().top -
          scroller.clientTop +
          scroller.scrollTop,
    padStart: px(style.paddingBlockStart || style.paddingTop),
    padEnd: px(style.paddingBlockEnd || style.paddingBottom),
    gap: px(style.rowGap),
    row: row !== undefined && row > 0 ? row : UNMEASURED.row,
  };
}

function same(a: Geometry, b: Geometry): boolean {
  return (
    a.margin === b.margin &&
    a.padStart === b.padStart &&
    a.padEnd === b.padEnd &&
    a.gap === b.gap &&
    a.row === b.row
  );
}

/** The indexes in sight with `aimed` among them, in order. */
function withAimed(indexes: number[], aimed: number | undefined): number[] {
  if (aimed === undefined || indexes.includes(aimed)) return indexes;
  return [...indexes, aimed].sort((a, b) => a - b);
}
