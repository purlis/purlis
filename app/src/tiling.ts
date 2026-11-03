// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Tile all, Undo and Focus this pane** (#4765, spec #4750 stories 76, 77 and 85): the moves
 * that rearrange a tab's own panes without adding or taking any.
 *
 * - **Tile all** evens a tab's panes into a grid of ⌈√n⌉ columns, filled row by row in reading
 *   order, with even shares: six panes are 3 × 2 at 333 then 500. Only the tab's own panes, so
 *   it never pulls in a chat nobody put there. Disabled at one pane.
 * - **Undo** puts the tree back, one step, while the tabs are as Tile all left them: it is the
 *   window's one Undo (`layoutUndo.ts`), which a drop and a split record too (#4763).
 * - **Zoom** shows one pane for now. The tree stays whole, so the core keeps the split.
 *
 * A zoom is per tab, and neither it nor Undo is told to the core.
 */
import { recorded } from "./layoutUndo";
import type { Layout, Tab, Tabs } from "./tabs";

type Pane = Extract<Layout, { kind: "pane" }>;

function panesOf(layout: Layout): Pane[] {
  return layout.kind === "pane" ? [layout] : layout.children.flatMap(panesOf);
}

/** Where each pane is drawn, as fractions of the tab: its left and top edges. */
function edges(layout: Layout, x = 0, y = 0, w = 1, h = 1): { pane: Pane; x: number; y: number }[] {
  if (layout.kind === "pane") return [{ pane: layout, x, y }];
  const share = (layout.share ?? 500) / 1000;
  const [a, b] = layout.children;
  return layout.direction === "row"
    ? [...edges(a, x, y, w * share, h), ...edges(b, x + w * share, y, w * (1 - share), h)]
    : [...edges(a, x, y, w, h * share), ...edges(b, x, y + h * share, w, h * (1 - share))];
}

/** Closer than this, two edges are the same line. */
const SAME = 1e-6;

/** The tab's panes in reading order: top to bottom, then left to right, as they are drawn. */
function inReadingOrder(layout: Layout): Pane[] {
  return edges(layout)
    .sort((one, other) => (Math.abs(one.y - other.y) > SAME ? one.y - other.y : one.x - other.x))
    .map((one) => one.pane);
}

/** `items` evened along `direction`: each split's first child takes its even part of the rest. */
function evenly(items: Layout[], direction: "row" | "column"): Layout {
  const [first, ...rest] = items;
  if (rest.length === 0) return first;
  return {
    kind: "split",
    direction,
    share: Math.round(1000 / items.length),
    children: [first, evenly(rest, direction)],
  };
}

/** `panes` as a grid of ⌈√n⌉ columns, filled row by row. */
function grid(panes: Pane[]): Layout {
  const columns = Math.ceil(Math.sqrt(panes.length));
  const rows: Layout[] = [];
  for (let at = 0; at < panes.length; at += columns)
    rows.push(evenly(panes.slice(at, at + columns), "row"));
  return evenly(rows, "column");
}

function withTab(tabs: Tabs, tab: Tab): Tabs {
  return { ...tabs, byId: { ...tabs.byId, [tab.id]: tab } };
}

function unzoomed(tab: Tab): Tab {
  const plain = { ...tab };
  delete plain.zoomed;
  return plain;
}

/** Whether tab `id` is split: more than one pane, which Tile all and a zoom need. */
export function isSplit(tabs: Tabs, id: number | undefined): boolean {
  const tab = id === undefined ? undefined : tabs.byId[id];
  return tab !== undefined && tab.layout.kind === "split";
}

/** Whether tab `id` has more than one pane to tile. */
export function canTileAll(tabs: Tabs, id: number): boolean {
  return isSplit(tabs, id);
}

/**
 * **Tile all** on tab `id`: its own panes, in reading order, into an even grid, the focused pane
 * kept focused and any zoom ended. Answers `tabs` itself at one pane, or for a tab not there.
 */
export function tileAll(tabs: Tabs, id: number): Tabs {
  if (!canTileAll(tabs, id)) return tabs;
  const tab = tabs.byId[id];
  const panes = inReadingOrder(tab.layout);
  const tiled = withTab(tabs, { ...unzoomed(tab), layout: grid(panes) });
  return recorded(tabs, tiled, `Tiled the tab's ${panes.length} panes.`);
}

/**
 * The pane zoomed in tab `id`, or nothing. **A zoom holds only while its pane has the focus**:
 * once the focus moves (a split, a pane focused, its pane gone) the pane drawn alone would not be
 * the one the keyboard and the next split act on, so it is no zoom. Nor is one in a tab left
 * with one pane.
 */
export function zoomedOf(tabs: Tabs, id: number): number | undefined {
  const tab = tabs.byId[id];
  if (tab?.zoomed === undefined || !isSplit(tabs, id) || tab.zoomed !== tab.focused)
    return undefined;
  return panesOf(tab.layout).some((one) => one.pane === tab.zoomed) ? tab.zoomed : undefined;
}

/**
 * **Focus this pane** in the tab in front: zooms its focused pane, or brings the split back when
 * a pane is zoomed. Answers `tabs` itself in a one-pane tab, where the key is the window's
 * Focus mode, and with no tab in front.
 */
export function zoomFocusedPane(tabs: Tabs): Tabs {
  const tab = tabs.inFront === undefined ? undefined : tabs.byId[tabs.inFront];
  if (tab === undefined || !isSplit(tabs, tab.id)) return tabs;
  if (zoomedOf(tabs, tab.id) !== undefined) return withTab(tabs, unzoomed(tab));
  return withTab(tabs, { ...tab, zoomed: tab.focused });
}

/**
 * **What tab `id` draws** from `layout`, its tree as drawn (`layoutShown`): the zoomed pane alone
 * while one is zoomed, else `layout` itself. The tree is never changed, so the split comes back
 * at its sizes.
 */
export function zoomedLayout(tabs: Tabs, id: number, layout: Layout): Layout {
  const pane = zoomedOf(tabs, id);
  return panesOf(layout).find((one) => one.pane === pane) ?? layout;
}
