// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  type Box,
  type DropContext,
  dropOn,
  floorAlong,
  judgeDrop,
  type Size,
  type Source,
  type Target,
} from "./paneDrop";
import { BAR, drawnOf, type Folded, RAIL } from "./paneFold";
import { shareOf, WHOLE } from "./laidOut";
import { type Content, layoutShown, type Layout, panes, sameContent, type Tabs } from "./tabs";
import { zoomedLayout } from "./tiling";

/**
 * **What a drag draws before the drop** (#5291, spec #5288): the panes where the drop would leave
 * them. It adds no rule: {@link wouldBe} is `dropOn`'s own result, and a place `judgeDrop` refuses
 * has none, so a preview can't promise what a drop won't do. Everything here is plain values; the
 * window measures the room and writes the boxes down (`paneHeld.ts`).
 */

/** What is between what a split and a fold draw, in CSS pixels, as the window draws them. */
export type Gaps = { separator: number; fold: number };

/** A folded side, as it would be drawn. */
export type RailDrawn = { box: Box; as: Folded["as"]; panes: number[] };

/** Where every pane of a layout is drawn in a room: whole, or folded to a rail or a bar. */
export type LaidBoxes = { panes: Map<number, Box>; rails: RailDrawn[] };

/**
 * **Where each pane of `layout` is drawn in `room`.** What folds is `drawnOf`'s to say, as it says
 * for the window, and a split's sides have their shares of what its separator leaves, kept within
 * both sides' minimum, as the separator keeps them.
 */
export function boxesOf(layout: Layout, focused: number, room: Box, gaps: Gaps): LaidBoxes {
  const { folds } = drawnOf(layout, focused, { width: room.width, height: room.height });
  const laid: LaidBoxes = { panes: new Map(), rails: [] };
  const lay = (one: Layout, box: Box, path: string): void => {
    if (one.kind === "pane") {
      laid.panes.set(one.pane, box);
      return;
    }
    const across = one.direction === "row";
    const whole = across ? box.width : box.height;
    const part = (from: number, length: number): Box =>
      across
        ? { ...box, left: box.left + from, width: length }
        : { ...box, top: box.top + from, height: length };
    const folded = folds[path];
    if (folded !== undefined) {
      const thin = across ? RAIL : BAR;
      const kept = whole - thin - gaps.fold;
      laid.rails.push({
        box: part(folded.side === 0 ? 0 : kept + gaps.fold, thin),
        as: folded.as,
        panes: folded.panes,
      });
      const keptSide = folded.side === 0 ? 1 : 0;
      lay(
        one.children[keptSide],
        part(folded.side === 0 ? thin + gaps.fold : 0, kept),
        `${path}${keptSide === 0 ? "a" : "b"}`,
      );
      return;
    }
    const along = whole - gaps.separator;
    const [a, b] = one.children;
    const wanted = Math.round((along * shareOf(one)) / WHOLE);
    const first = Math.min(
      Math.max(wanted, floorAlong(a, one.direction)),
      along - floorAlong(b, one.direction),
    );
    lay(a, part(0, first), `${path}a`);
    lay(b, part(first + gaps.separator, along - first), `${path}b`);
  };
  lay(layout, room, "");
  return laid;
}

/** What the tab in front would draw after a drop: its tree as drawn, and the pane with the focus. */
export type WouldBe = { layout: Layout; focused: number };

/**
 * **What dropping `from` on `to` would leave in the tab in front**, or nothing where the panes
 * have nothing to show: a place that is refused, what is held over its own place, and the strip,
 * where a line says where the tab goes and the panes stay put. It asks the drop model, and the
 * tabs it is given are not changed.
 */
export function wouldBe(
  tabs: Tabs,
  from: Source,
  to: Target,
  ctx: DropContext,
  size?: Size,
): WouldBe | undefined {
  if (to.kind === "strip" || !judgeDrop(tabs, from, to, ctx, size).ok) return undefined;
  const after = dropOn(tabs, from, to, ctx, size);
  const front = tabs.inFront;
  const tab = front === undefined ? undefined : after.byId[front];
  if (front === undefined || tab === undefined) return undefined;
  return {
    layout: zoomedLayout(after, front, layoutShown(after, front) ?? tab.layout),
    focused: tab.focused,
  };
}

/** A pane with no frame on screen to move: a chat from the list, or one that comes back unfolded. */
export type Arriving = { pane: number; box: Box; content: Content };

/**
 * **What each pane does in the preview.** A pane on screen that would still show the same thing
 * moves to its box (`frames`). One that would not be drawn whole, or whose place something else
 * takes, is hidden while the preview lasts (`gone`), and keeps running. What has no frame on
 * screen is drawn as one arriving; what would fold, as a rail.
 */
export type Plan = {
  frames: Map<number, Box>;
  arriving: Arriving[];
  gone: number[];
  rails: RailDrawn[];
};

/** **The plan for `after`**, given what is on screen now: each drawn pane's number and content. */
export function planOf(
  after: WouldBe,
  onScreen: ReadonlyMap<number, Content>,
  room: Box,
  gaps: Gaps,
): Plan {
  const laid = boxesOf(after.layout, after.focused, room, gaps);
  const frames = new Map<number, Box>();
  const arriving: Arriving[] = [];
  for (const { pane, content } of panes(after.layout)) {
    const box = laid.panes.get(pane);
    if (box === undefined) continue;
    const now = onScreen.get(pane);
    if (now !== undefined && sameContent(now, content)) frames.set(pane, box);
    else arriving.push({ pane, box, content });
  }
  const gone = [...onScreen.keys()].filter((pane) => !frames.has(pane));
  return { frames, arriving, gone, rails: laid.rails };
}

/** How a terminal's text sits in a pane: its cell, and what the pane draws around its grid. */
export type TerminalCells = {
  width: number;
  height: number;
  /** All the width that is not columns: the scrollbar's, and the pane's own around the terminal. */
  aroundWidth: number;
  /** All the height that is not rows: the head, the band and the foot. */
  aroundHeight: number;
};

/** A terminal's size, in its own cells. */
export type Grid = { columns: number; rows: number };

/** The least a terminal's fit gives it (`@xterm/addon-fit`). */
const LEAST: Grid = { columns: 2, rows: 1 };

/**
 * **The size a terminal would take in a pane of `box`**: as its fit reckons it, whole cells of what
 * is left, so the label says what the program will be told.
 */
export function gridIn(box: Size, cells: TerminalCells): Grid {
  return {
    columns: Math.max(LEAST.columns, Math.floor((box.width - cells.aroundWidth) / cells.width)),
    rows: Math.max(LEAST.rows, Math.floor((box.height - cells.aroundHeight) / cells.height)),
  };
}
