// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  type Backgrounded,
  type Content,
  closeTab,
  type FiledIn,
  type Layout,
  nothingBackgrounded,
  nothingPinned,
  type Pinned,
  showIn,
  type Tab,
  type Tabs,
  workspaceOf,
} from "./tabs";

/**
 * **Dragging panes: split, swap and move** (#4763), as the prototype's verdict ruled
 * (`prototype/pane-drag` variant A, resolved on #4583). Where the pointer is in a pane picks the
 * move: the outer quarter by an edge splits on that side, the middle swaps. This is the whole of
 * what a drop does and why one is refused, on plain values; the window only measures where the
 * pointer is ({@link placeAt}) and draws the answer before the drop.
 *
 * - **On an edge**, the source leaves its place, its sibling takes the split, and the source is
 *   the split's first child for left and top, its second for right and bottom, at an even share.
 * - **In the middle**, pane on pane swaps; onto an empty pane the source fills it and its old
 *   pane closes; a sidebar chat replaces what the pane shows, and the chat it showed keeps
 *   running in the sidebar (D1).
 * - **On the strip**, a pane becomes a tab of its own where the line is, and a tab moves there.
 * - **A chat already on screen acts as its pane**, so it moves rather than showing twice (D1).
 * - **A pane's last chat leaving its tab closes the tab**, unless it is its strip's last tab,
 *   which keeps an empty pane (#4587).
 * - **A moved pane keeps its number**, so its terminal is the same one wherever it lands.
 */

/** Where in a pane a drop lands: an edge splits on that side, the middle swaps. */
export type Place = "left" | "right" | "top" | "bottom" | "middle";

/** What is dragged: a pane, a sidebar row's chat, or a tab off the strip. */
export type Source =
  | { kind: "pane"; tab: number; pane: number }
  | { kind: "chat"; session: number; chat: string }
  | { kind: "tab"; tab: number };

/** Where it is dropped: a place in a pane, or the strip before the tab at `at`. */
export type Target =
  { kind: "pane"; tab: number; pane: number; place: Place } | { kind: "strip"; at: number };

/** A pane's size on screen, in CSS pixels. */
export type Size = { width: number; height: number };

/**
 * **The smallest a pane may be after a split**: about 40 columns by 8 rows of the terminal
 * (#4583). A split that would leave a half under it is refused, and a separator stops at it.
 */
export const PANE_FLOOR: Size = { width: 300, height: 140 };

/** What a drop is judged against that the tabs alone do not say. */
export type DropContext = {
  /** The workspace a chat is filed in. */
  filedIn: FiledIn;
  /** What a pane's content is called in a sentence. */
  name: (content: Content) => string;
  pinned?: Pinned;
  background?: Backgrounded;
};

/** What a drop would do, said, or why it is refused. */
export type Verdict = { ok: true; says: string } | { ok: false; why: string };

/** How far in from an edge, as a share of the pane, a drop splits rather than swaps. */
export const EDGE = 0.25;

/**
 * **The place a point in a pane picks**: the nearest edge when it is within its outer quarter,
 * else the middle. `x` and `y` are the point's share of the pane's width and height, 0 to 1.
 */
export function placeAt(x: number, y: number): Place {
  const edges: [Place, number][] = [
    ["left", x],
    ["right", 1 - x],
    ["top", y],
    ["bottom", 1 - y],
  ];
  const [place, distance] = edges.reduce((near, one) => (one[1] < near[1] ? one : near));
  return distance < EDGE ? place : "middle";
}

const SIDE_WORD: Record<Exclude<Place, "middle">, string> = {
  left: "left of",
  right: "right of",
  top: "above",
  bottom: "below",
};

/**
 * **What dropping `from` on `to` would do, or why it can't.** `size` is the target pane's size
 * on screen; without it the minimum size is not checked.
 */
export function judgeDrop(
  tabs: Tabs,
  from: Source,
  to: Target,
  ctx: DropContext,
  size?: Size,
): Verdict {
  const source = normal(tabs, from, to);
  if (source.kind === "pane") {
    const leaf = leafOf(tabs, source.tab, source.pane);
    if (leaf === undefined) return refused("That pane has closed.");
    if (leaf.content.kind === "empty")
      return refused("An empty pane has nothing to move. Close it instead.");
  }
  if (source.kind === "tab" && tabs.byId[source.tab] === undefined)
    return refused("That tab has closed.");
  const name = sourceName(tabs, source, ctx);
  if (to.kind === "strip")
    return said(source.kind === "tab" ? "Move the tab here." : `A new tab with ${name}.`);

  const target = leafOf(tabs, to.tab, to.pane);
  if (target === undefined) return refused("That pane has closed.");
  if (source.kind === "pane" && source.pane === to.pane)
    return refused("That's the pane being moved.");
  if (source.kind === "tab" && source.tab === to.tab) return refused("That's the tab being moved.");
  const there = ctx.name(target.content);

  let says: string;
  if (to.place === "middle") {
    if (source.kind === "tab")
      return refused("A tab of several panes goes beside a pane, not into one.");
    if (target.content.kind === "empty") says = `Put ${name} in this empty pane.`;
    else if (source.kind === "pane") says = `Swap ${name} and ${there}.`;
    else says = `Show ${name} here. ${there} keeps running in the sidebar.`;
  } else {
    if (size !== undefined) {
      const across = to.place === "left" || to.place === "right";
      if (across ? size.width / 2 < PANE_FLOOR.width : size.height / 2 < PANE_FLOOR.height)
        return refused(
          across
            ? "Too narrow: each half would be under 40 columns."
            : "Too short: each half would be under 8 rows.",
        );
    }
    const what = source.kind === "tab" ? `The tab ${name}` : name;
    says = `${what} ${SIDE_WORD[to.place]} ${there}.`;
  }

  // Another workspace's chat may land in a pane, by drag only (#4583), and stays filed in its
  // own. A tab's strip is still read from its first pane (#4755 D9) until the tab keeps a
  // workspace of its own (#4757), so a drop that would take that first place is refused: it
  // would carry the whole tab to the other strip.
  const home = homeOf(tabs, source, ctx);
  const strip = workspaceOf(tabs, to.tab, ctx.filedIn);
  if (home !== undefined && strip !== undefined && home !== strip) {
    const after = apply(tabs, source, to, ctx);
    if (workspaceOf(after, to.tab, ctx.filedIn) !== strip)
      return refused(`${name} works in ${home}. Drop it right of or below ${there}.`);
    says = `${says} It stays filed in ${home}.`;
  }
  return said(says);
}

/** **`tabs` after dropping `from` on `to`**, or `tabs` itself when the drop is refused. */
export function dropOn(tabs: Tabs, from: Source, to: Target, ctx: DropContext, size?: Size): Tabs {
  if (!judgeDrop(tabs, from, to, ctx, size).ok) return tabs;
  return apply(tabs, normal(tabs, from, to), to, ctx);
}

const refused = (why: string): Verdict => ({ ok: false, why });
const said = (says: string): Verdict => ({ ok: true, says });

/**
 * The source as the drop acts on it: a sidebar chat already on screen is its pane; a tab of one
 * pane dropped on a pane is that pane; a pane alone in its tab dropped on the strip is its tab.
 */
function normal(tabs: Tabs, from: Source, to: Target): Source {
  if (from.kind === "chat") {
    for (const id of tabs.order) {
      const on = leaves(tabs.byId[id].layout).find(
        ({ content }) => content.kind === "session" && content.session === from.session,
      );
      if (on !== undefined) return normal(tabs, { kind: "pane", tab: id, pane: on.pane }, to);
    }
    return from;
  }
  const tab = tabs.byId[from.tab];
  if (tab === undefined) return from;
  const all = leaves(tab.layout);
  if (from.kind === "tab" && to.kind === "pane" && all.length === 1)
    return { kind: "pane", tab: from.tab, pane: all[0].pane };
  if (from.kind === "pane" && to.kind === "strip" && all.length === 1)
    return { kind: "tab", tab: from.tab };
  return from;
}

function sourceName(tabs: Tabs, source: Source, ctx: DropContext): string {
  if (source.kind === "chat")
    return ctx.name({ kind: "session", session: source.session, chat: source.chat });
  const tab = tabs.byId[source.tab];
  const leaf =
    source.kind === "pane" ? leafOf(tabs, source.tab, source.pane) : leaves(tab.layout)[0];
  return leaf === undefined ? "" : ctx.name(leaf.content);
}

/** The workspace what is dragged is filed in. */
function homeOf(tabs: Tabs, source: Source, ctx: DropContext): string | undefined {
  if (source.kind === "chat") return ctx.filedIn(source.session);
  if (source.kind === "tab") return workspaceOf(tabs, source.tab, ctx.filedIn);
  const content = leafOf(tabs, source.tab, source.pane)?.content;
  if (content === undefined) return undefined;
  return content.kind === "session" ? ctx.filedIn(content.session) : content.workspace;
}

/** The drop, already judged. */
function apply(tabs: Tabs, source: Source, to: Target, ctx: DropContext): Tabs {
  if (to.kind === "strip") return toStrip(tabs, source, to.at, ctx);
  if (to.place === "middle") {
    if (source.kind === "chat") {
      const shown = showIn(tabs, to.tab, to.pane, {
        kind: "session",
        session: source.session,
        chat: source.chat,
      });
      return inFront(shown, to.tab, to.pane);
    }
    if (source.kind === "pane") {
      const target = leafOf(tabs, to.tab, to.pane);
      if (target?.content.kind === "empty") {
        const lifted = lift(tabs, source, ctx);
        const placed = edit(lifted.tabs, to.tab, (layout) => swapIn(layout, to.pane, lifted.moved));
        return inFront(carry(placed, lifted.shows, to.tab), to.tab, firstPane(lifted.moved));
      }
      return swap(tabs, source, to.tab, to.pane);
    }
    return tabs;
  }
  const lifted = lift(tabs, source, ctx);
  const direction = to.place === "left" || to.place === "right" ? "row" : "column";
  const first = to.place === "left" || to.place === "top";
  const placed = edit(lifted.tabs, to.tab, (layout) =>
    swapIn(layout, to.pane, (found) => ({
      kind: "split",
      direction,
      children: first ? [lifted.moved, found] : [found, lifted.moved],
    })),
  );
  return inFront(carry(placed, lifted.shows, to.tab), to.tab, firstPane(lifted.moved));
}

/** A tab made of `source`, or `source`'s tab moved, before the tab at `at`. */
function toStrip(tabs: Tabs, source: Source, at: number, ctx: DropContext): Tabs {
  const before = tabs.order[at];
  if (source.kind === "tab") {
    const order = tabs.order.filter((id) => id !== source.tab);
    const index = before === undefined ? order.length : order.indexOf(before);
    order.splice(index < 0 ? order.length : index, 0, source.tab);
    return { ...tabs, order, inFront: source.tab };
  }
  const lifted = lift(tabs, source, ctx);
  const next = lifted.tabs;
  const id = next.named.tabs + 1;
  const name = ctx.name(leaves(lifted.moved)[0].content);
  const tab: Tab = {
    id,
    name,
    defaultName: name,
    layout: lifted.moved,
    focused: firstPane(lifted.moved),
  };
  const order = [...next.order];
  const index = before === undefined ? order.length : order.indexOf(before);
  order.splice(index < 0 ? order.length : index, 0, id);
  const made = {
    ...next,
    byId: { ...next.byId, [id]: tab },
    order,
    inFront: id,
    named: { ...next.named, tabs: id },
  };
  return carry(made, lifted.shows, id);
}

type Lifted = {
  tabs: Tabs;
  moved: Layout;
  /** What the moved panes showed in place of their own chat (`Tab.shows`). */
  shows: Record<number, number>;
};

/** Takes `source` out of where it is: a pane leaves its tree, a tab its strip, and a sidebar
 *  chat becomes a new pane. */
function lift(tabs: Tabs, source: Source, ctx: DropContext): Lifted {
  if (source.kind === "chat") {
    const pane = tabs.named.panes + 1;
    return {
      tabs: { ...tabs, named: { ...tabs.named, panes: pane } },
      moved: {
        kind: "pane",
        pane,
        content: { kind: "session", session: source.session, chat: source.chat },
      },
      shows: {},
    };
  }
  const tab = tabs.byId[source.tab];
  const moved =
    source.kind === "tab" ? tab.layout : (leafOf(tabs, source.tab, source.pane) ?? tab.layout);
  const numbers = new Set(leaves(moved).map(({ pane }) => pane));
  const shows = Object.fromEntries(
    Object.entries(tab.shows ?? {}).filter(([pane]) => numbers.has(Number(pane))),
  ) as Record<number, number>;
  const left = source.kind === "tab" ? undefined : without(tab.layout, source.pane);
  if (left !== undefined) {
    const kept = Object.fromEntries(
      Object.entries(tab.shows ?? {}).filter(([pane]) => !numbers.has(Number(pane))),
    ) as Record<number, number>;
    const rest: Tab = {
      ...tab,
      layout: left,
      focused: numbers.has(tab.focused) ? firstPane(left) : tab.focused,
    };
    delete rest.shows;
    if (Object.keys(kept).length > 0) rest.shows = kept;
    return { tabs: { ...tabs, byId: { ...tabs.byId, [tab.id]: rest } }, moved, shows };
  }
  // The tab is left with nothing: it closes, unless it is its strip's last tab.
  const strip = workspaceOf(tabs, tab.id, ctx.filedIn);
  const another = tabs.order.some(
    (id) => id !== tab.id && workspaceOf(tabs, id, ctx.filedIn) === strip,
  );
  if (another || strip === undefined) {
    const closed = closeTab(
      tabs,
      tab.id,
      ctx.filedIn,
      ctx.pinned ?? nothingPinned,
      ctx.background ?? nothingBackgrounded,
    );
    return { tabs: closed, moved, shows };
  }
  const pane = tabs.named.panes + 1;
  const emptied: Tab = {
    ...tab,
    layout: { kind: "pane", pane, content: { kind: "empty", workspace: strip } },
    focused: pane,
  };
  delete emptied.shows;
  return {
    tabs: {
      ...tabs,
      byId: { ...tabs.byId, [tab.id]: emptied },
      named: { ...tabs.named, panes: pane },
    },
    moved,
    shows,
  };
}

/** Pane `a` of tab `from.tab` and pane `b` of tab `tab` trade places, each keeping its number. */
function swap(tabs: Tabs, from: { tab: number; pane: number }, tab: number, b: number): Tabs {
  const one = leafOf(tabs, from.tab, from.pane);
  const other = leafOf(tabs, tab, b);
  if (one === undefined || other === undefined) return tabs;
  const exchange = (layout: Layout): Layout =>
    layout.kind === "pane"
      ? layout.pane === one.pane
        ? other
        : layout.pane === other.pane
          ? one
          : layout
      : {
          ...layout,
          children: layout.children.map(exchange) as [Layout, Layout],
        };
  let next = edit(tabs, from.tab, exchange);
  if (from.tab !== tab) {
    next = edit(next, tab, exchange);
    // What each pane showed in place of its own chat goes with it.
    const shownOne = tabs.byId[from.tab].shows?.[one.pane];
    const shownOther = tabs.byId[tab].shows?.[other.pane];
    next = moveShows(next, from.tab, one.pane, other.pane, shownOther);
    next = moveShows(next, tab, other.pane, one.pane, shownOne);
    const source = next.byId[from.tab];
    if (source.focused === one.pane)
      next = { ...next, byId: { ...next.byId, [from.tab]: { ...source, focused: other.pane } } };
  }
  return inFront(next, tab, one.pane);
}

/** Tab `id`'s `shows` without pane `gone`'s entry, and with `shown` for pane `came`. */
function moveShows(
  tabs: Tabs,
  id: number,
  gone: number,
  came: number,
  shown: number | undefined,
): Tabs {
  const tab = tabs.byId[id];
  const shows = Object.fromEntries(
    Object.entries(tab.shows ?? {}).filter(([pane]) => Number(pane) !== gone),
  ) as Record<number, number>;
  if (shown !== undefined) shows[came] = shown;
  const next: Tab = { ...tab };
  delete next.shows;
  if (Object.keys(shows).length > 0) next.shows = shows;
  return { ...tabs, byId: { ...tabs.byId, [id]: next } };
}

/** Tab `id` with `shows` added: the moved panes' tasks go with them. */
function carry(tabs: Tabs, shows: Record<number, number>, id: number): Tabs {
  if (Object.keys(shows).length === 0) return tabs;
  const tab = tabs.byId[id];
  return { ...tabs, byId: { ...tabs.byId, [id]: { ...tab, shows: { ...tab.shows, ...shows } } } };
}

/** Tab `id` in front, with pane `pane` focused. */
function inFront(tabs: Tabs, id: number, pane: number): Tabs {
  const tab = tabs.byId[id];
  if (tab === undefined) return tabs;
  return { ...tabs, byId: { ...tabs.byId, [id]: { ...tab, focused: pane } }, inFront: id };
}

function edit(tabs: Tabs, id: number, change: (layout: Layout) => Layout): Tabs {
  const tab = tabs.byId[id];
  return { ...tabs, byId: { ...tabs.byId, [id]: { ...tab, layout: change(tab.layout) } } };
}

type Leaf = Extract<Layout, { kind: "pane" }>;

function leaves(layout: Layout): Leaf[] {
  return layout.kind === "pane" ? [layout] : layout.children.flatMap(leaves);
}

function leafOf(tabs: Tabs, tab: number, pane: number): Leaf | undefined {
  const found = tabs.byId[tab];
  return found === undefined ? undefined : leaves(found.layout).find((one) => one.pane === pane);
}

const firstPane = (layout: Layout): number => leaves(layout)[0].pane;

/** `layout` with pane `pane` replaced by `by` (a layout, or one made from the pane found). */
function swapIn(layout: Layout, pane: number, by: Layout | ((found: Leaf) => Layout)): Layout {
  if (layout.kind === "pane")
    return layout.pane === pane ? (typeof by === "function" ? by(layout) : by) : layout;
  return {
    ...layout,
    children: layout.children.map((child) => swapIn(child, pane, by)) as [Layout, Layout],
  };
}

/** `layout` without pane `pane`: its sibling takes the split's place. */
function without(layout: Layout, pane: number): Layout | undefined {
  if (layout.kind === "pane") return layout.pane === pane ? undefined : layout;
  const [first, second] = layout.children.map((child) => without(child, pane));
  if (!first) return second;
  if (!second) return first;
  return { ...layout, children: [first, second] };
}

/** A box on screen, in CSS pixels. */
export type Box = { left: number; top: number; width: number; height: number };

/**
 * **Where the result of a drop is drawn before it** (#4583): the half of the pane the source
 * would take for an edge, the whole pane for its middle.
 */
export function previewOf(pane: Box, place: Place): Box {
  const halfWidth = pane.width / 2;
  const halfHeight = pane.height / 2;
  switch (place) {
    case "left":
      return { ...pane, width: halfWidth };
    case "right":
      return { ...pane, left: pane.left + halfWidth, width: halfWidth };
    case "top":
      return { ...pane, height: halfHeight };
    case "bottom":
      return { ...pane, top: pane.top + halfHeight, height: halfHeight };
    case "middle":
      return pane;
  }
}

/**
 * **The smallest a side of a split may be, in pixels, along `direction`** (#4763): a pane's
 * floor, and a split along the same direction the sum of its sides', since each of them keeps
 * its own. A separator stops there, so it keeps the floor a drop keeps.
 */
export function floorAlong(layout: Layout, direction: "row" | "column"): number {
  if (layout.kind === "pane") return direction === "row" ? PANE_FLOOR.width : PANE_FLOOR.height;
  const [a, b] = layout.children.map((child) => floorAlong(child, direction));
  return layout.direction === direction ? a + b : Math.max(a, b);
}

/**
 * **The tabs a drop's Undo puts back** (#4583: Undo after every layout change, ⌘Z), or nothing
 * once the tabs have moved on from what the drop left: what was dragged since, or a chat opened
 * or closed, is not taken back by an Undo it never saw. Focus and the front tab moving are no
 * such change.
 */
export function undone(now: Tabs, before: Tabs, after: Tabs): Tabs | undefined {
  const same =
    now.order.length === after.order.length &&
    now.order.every(
      (id, at) => after.order[at] === id && now.byId[id]?.layout === after.byId[id]?.layout,
    );
  if (!same) return undefined;
  // Numbers handed out since stay handed out: a pane or a tab never reuses one.
  return { ...before, named: now.named };
}
