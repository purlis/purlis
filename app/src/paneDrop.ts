// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  type Content,
  closeTab,
  contentsOf,
  EMPTY_TAB,
  type FiledIn,
  findView,
  type Layout,
  nothingPinned,
  isOneJob,
  ONE_JOB_STAYS,
  type Pane,
  panes,
  type Pinned,
  replace,
  sameContent,
  showIn,
  type Tab,
  type Tabs,
  without,
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
 * - **A tab's name is its first pane's** (`tabs.renameTab`), so a drop that puts something else
 *   first names the tab for it (#5283). A chat's own name never changes for a drop.
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
  /**
   * What a tab is called when `chat` is its first pane: the name it says, and what that goes back
   * to when a given name is taken off (`Tab.defaultName`). `name` for both where this says
   * nothing: where it is absent, or for a chat it does not know, such as one with no row yet.
   */
  tabName?: (chat: Extract<Content, { kind: "session" }>) => TabName | undefined;
  pinned?: Pinned;
};

type TabName = { name: string; defaultName: string };

/** What a drop would do, said, or why it is refused. */
export type Verdict =
  /** `says` what it would do, before the drop; `did` what it did, after it. */
  | { ok: true; says: string; did: string }
  /** `overItself`: what is dragged is over its own place, which is no mistake to warn about:
   *  letting go there moves nothing (#5284). */
  | { ok: false; why: string; overItself?: true };

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
  const source = asDropped(tabs, from, to);
  if (source.kind === "pane") {
    const pane = paneAt(tabs, source);
    if (pane === undefined) return refused("That pane has closed.");
    // First: over its own place nothing moves, whatever it is that could not have moved (#5284).
    if (to.kind === "pane" && source.pane === to.pane)
      return overItself("That's the pane being moved. Drop it on another pane.");
    if (pane.content.kind === "empty")
      return refused("An empty pane has nothing to move. Close it instead.");
    // A one-job program stays beside the pane it opened from (#4761; spec #4750 story 78).
    if (isOneJob(pane.content)) return refused(ONE_JOB_STAYS);
  }
  if (source.kind === "tab" && tabs.byId[source.tab] === undefined)
    return refused("That tab has closed.");
  const name = sourceName(tabs, source, ctx);
  const home = homeOf(tabs, source, ctx);

  if (to.kind === "strip") {
    if (source.kind === "tab") return said("Move the tab here.", "Moved the tab.");
    // Another workspace's chat makes its tab on that workspace's strip, where it is filed.
    const strip =
      tabs.inFront === undefined ? undefined : workspaceOf(tabs, tabs.inFront, ctx.filedIn);
    const elsewhere = home !== undefined && strip !== undefined && home !== strip;
    return elsewhere
      ? said(
          `A new tab with ${name}, on ${home}'s strip.`,
          `Made a new tab with ${name}, on ${home}'s strip.`,
        )
      : said(`A new tab with ${name}.`, `Made a new tab with ${name}.`);
  }

  const target = paneAt(tabs, to);
  if (target === undefined) return refused("That pane has closed.");
  if (source.kind === "tab" && source.tab === to.tab)
    return overItself("That's the tab being moved. Drop it on a pane in another tab.");
  const there = ctx.name(target.content);

  let verdict: { says: string; did: string };
  if (to.place === "middle") {
    if (source.kind === "tab")
      return refused("A tab of several panes goes beside a pane, not into one.");
    // Its edges still take a split, but nothing swaps with it or takes its place (#4761).
    if (isOneJob(target.content))
      return refused(`${there} goes when its program exits. Drop beside it instead.`);
    if (target.content.kind === "empty")
      verdict = {
        says: `Put ${name} in this empty pane.`,
        did: `Put ${name} in the empty pane.`,
      };
    else if (source.kind === "pane")
      verdict = { says: `Swap ${name} and ${there}.`, did: `Swapped ${name} and ${there}.` };
    else
      verdict = {
        says: `Show ${name} here. ${there} keeps running in the sidebar.`,
        did: `Showing ${name} in place of ${there}, which keeps running in the sidebar.`,
      };
  } else {
    const across = to.place === "left" || to.place === "right";
    // Each half must hold the panes put in it: the dragged tab's own, or one pane.
    const moved = source.kind === "tab" ? tabs.byId[source.tab].layout : undefined;
    const direction = across ? "row" : "column";
    const needs = Math.max(
      across ? PANE_FLOOR.width : PANE_FLOOR.height,
      moved === undefined ? 0 : floorAlong(moved, direction),
    );
    if (size !== undefined && (across ? size.width : size.height) / 2 < needs)
      return refused(
        across
          ? "Too narrow: a pane here would be under about 40 columns. Drop it on a wider pane."
          : "Too short: a pane here would be under about 8 rows. Drop it on a taller pane.",
      );
    // A chat's name is said as it is written, even first in a sentence: it is a name.
    const what = source.kind === "tab" ? `the tab ${name}` : name;
    verdict = {
      says: `${source.kind === "tab" ? `The tab ${name}` : name} ${SIDE_WORD[to.place]} ${there}.`,
      did: `Put ${what} ${SIDE_WORD[to.place]} ${there}.`,
    };
  }

  // Another workspace's chat may land in a pane, by drag only (#4583), and stays filed in its
  // own. A tab's strip is read from its first pane (#4755 D9) until the tab keeps a workspace of
  // its own (#4757), so a drop that would take that first place is refused: it would carry the
  // whole tab to the other strip.
  const strip = workspaceOf(tabs, to.tab, ctx.filedIn);
  if (home !== undefined && strip !== undefined && home !== strip) {
    const after = apply(tabs, source, to, ctx);
    if (workspaceOf(after, to.tab, ctx.filedIn) !== strip)
      return refused(`${name} works in ${home}. Drop it right of or below ${there}.`);
    const filed = ` It stays filed in ${home}.`;
    return said(verdict.says + filed, verdict.did + filed);
  }
  return said(verdict.says, verdict.did);
}

/** **`tabs` after dropping `from` on `to`**, or `tabs` itself when the drop is refused. */
export function dropOn(tabs: Tabs, from: Source, to: Target, ctx: DropContext, size?: Size): Tabs {
  if (!judgeDrop(tabs, from, to, ctx, size).ok) return tabs;
  return namedForFirstPanes(tabs, apply(tabs, asDropped(tabs, from, to), to, ctx), ctx);
}

/**
 * **What a tab with `content` in its first pane is called**, as `tabs` stands before the drop. A
 * view keeps the title of the tab it led; a view beside another pane has no title of its own, so
 * it says its own word, as the layout records it (`laidOut.nodeOf`), never the name of the chat
 * beside it, which the record would then keep as its title.
 */
function tabNameOf(content: Content, tabs: Tabs, ctx: DropContext): TabName {
  if (content.kind === "empty") return { name: EMPTY_TAB, defaultName: EMPTY_TAB };
  if (content.kind === "view") {
    const at = findView(tabs, content.view);
    if (at !== undefined && contentsOf(tabs, at.tab)[0]?.pane === at.pane) {
      const { name, defaultName } = tabs.byId[at.tab];
      return { name, defaultName };
    }
    return { name: content.view.view, defaultName: content.view.view };
  }
  const named = ctx.tabName?.(content);
  if (named !== undefined) return named;
  const name = ctx.name(content);
  return { name, defaultName: name };
}

/** `after`, with each tab whose first pane shows something else than `before` named for it. */
function namedForFirstPanes(before: Tabs, after: Tabs, ctx: DropContext): Tabs {
  const byId = { ...after.byId };
  for (const id of after.order) {
    const was = contentsOf(before, id)[0]?.content;
    const is = contentsOf(after, id)[0]?.content;
    if (was === undefined || is === undefined || sameContent(was, is)) continue;
    byId[id] = { ...byId[id], ...tabNameOf(is, before, ctx) };
  }
  return { ...after, byId };
}

const refused = (why: string): Verdict => ({ ok: false, why });
const overItself = (why: string): Verdict => ({ ok: false, why, overItself: true });
const said = (says: string, did: string): Verdict => ({ ok: true, says, did });

/**
 * **What a drop acts on**, which is not always what was picked up: a sidebar chat already on
 * screen is its pane; a tab of one pane dropped on a pane is that pane; a pane alone in its tab
 * dropped on the strip is its tab.
 */
function asDropped(tabs: Tabs, from: Source, to: Target): Source {
  if (from.kind === "chat") {
    for (const id of tabs.order) {
      const on = panes(tabs.byId[id].layout).find(
        ({ content }) => content.kind === "session" && content.session === from.session,
      );
      if (on !== undefined) return asDropped(tabs, { kind: "pane", tab: id, pane: on.pane }, to);
    }
    return from;
  }
  const tab = tabs.byId[from.tab];
  if (tab === undefined) return from;
  const all = panes(tab.layout);
  if (from.kind === "tab" && to.kind === "pane" && all.length === 1)
    return { kind: "pane", tab: from.tab, pane: all[0].pane };
  if (from.kind === "pane" && to.kind === "strip" && all.length === 1)
    return { kind: "tab", tab: from.tab };
  return from;
}

/**
 * **The pane that moves when `source` is dropped on a pane**, if a pane does: the pane itself,
 * or the pane a sidebar chat is already on screen in (D1). It takes no drop of its own, so
 * placement mode shows it no targets (#4764).
 */
export function heldPane(tabs: Tabs, source: Source): At | undefined {
  if (source.kind === "tab") return undefined;
  // What moves when it lands on a pane: `asDropped` reads only that the target is a pane.
  const dropped = asDropped(tabs, source, ON_A_PANE);
  return dropped.kind === "pane" ? { tab: dropped.tab, pane: dropped.pane } : undefined;
}

/** Any drop on a pane, for asking what moves there. */
const ON_A_PANE: Target = { kind: "pane", tab: 0, pane: 0, place: "middle" };

/** **What a dragged thing is called** in a sentence: its chat, its view, or its tab's first. */
export function sourceName(tabs: Tabs, source: Source, ctx: DropContext): string {
  if (source.kind === "chat")
    return ctx.name({ kind: "session", session: source.session, chat: source.chat });
  const tab = tabs.byId[source.tab];
  if (tab === undefined) return "";
  const pane = source.kind === "pane" ? paneAt(tabs, source) : panes(tab.layout)[0];
  return pane === undefined ? "" : ctx.name(pane.content);
}

/** The workspace what is dragged is filed in. */
function homeOf(tabs: Tabs, source: Source, ctx: DropContext): string | undefined {
  if (source.kind === "chat") return ctx.filedIn(source.session);
  if (source.kind === "tab") return workspaceOf(tabs, source.tab, ctx.filedIn);
  const content = paneAt(tabs, source)?.content;
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
      if (paneAt(tabs, to)?.content.kind === "empty") {
        const lifted = lift(tabs, source, ctx);
        const placed = edit(lifted.tabs, to.tab, (layout) =>
          replace(layout, to.pane, () => lifted.moved),
        );
        return inFront(carry(placed, lifted.shows, to.tab), to.tab, firstPane(lifted.moved));
      }
      return swap(tabs, source, to);
    }
    return tabs;
  }
  const lifted = lift(tabs, source, ctx);
  const direction = to.place === "left" || to.place === "right" ? "row" : "column";
  const first = to.place === "left" || to.place === "top";
  const placed = edit(lifted.tabs, to.tab, (layout) =>
    replace(layout, to.pane, (found) => ({
      kind: "split",
      direction,
      children: first ? [lifted.moved, found] : [found, lifted.moved],
    })),
  );
  return inFront(carry(placed, lifted.shows, to.tab), to.tab, firstPane(lifted.moved));
}

/** A tab made of `source`, or `source`'s tab moved, before the tab at `at`. */
function toStrip(tabs: Tabs, source: Source, at: number, ctx: DropContext): Tabs {
  // The tab it goes before: the one at the line, or the next, when that is the tab being moved.
  const before = tabs.order.slice(at).find((id) => source.kind !== "tab" || id !== source.tab);
  const placed = (order: number[], id: number): number[] => {
    const index = before === undefined ? -1 : order.indexOf(before);
    return index < 0 ? [...order, id] : [...order.slice(0, index), id, ...order.slice(index)];
  };
  if (source.kind === "tab")
    return {
      ...tabs,
      order: placed(
        tabs.order.filter((id) => id !== source.tab),
        source.tab,
      ),
      inFront: source.tab,
    };
  const lifted = lift(tabs, source, ctx);
  const next = lifted.tabs;
  const id = next.named.tabs + 1;
  const tab: Tab = {
    id,
    ...tabNameOf(panes(lifted.moved)[0].content, tabs, ctx),
    layout: lifted.moved,
    focused: firstPane(lifted.moved),
  };
  const made = {
    ...next,
    byId: { ...next.byId, [id]: tab },
    order: placed(next.order, id),
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
  const moved = source.kind === "tab" ? tab.layout : (paneAt(tabs, source) ?? tab.layout);
  const numbers = new Set(panes(moved).map(({ pane }) => pane));
  const shows = showsWhere(tab, (pane) => numbers.has(pane));
  const left = source.kind === "tab" ? undefined : without(tab.layout, source.pane);
  if (left !== undefined) {
    const rest = withShows(
      { ...tab, layout: left, focused: numbers.has(tab.focused) ? firstPane(left) : tab.focused },
      showsWhere(tab, (pane) => !numbers.has(pane)),
    );
    return { tabs: { ...tabs, byId: { ...tabs.byId, [tab.id]: rest } }, moved, shows };
  }
  // The tab is left with nothing: it closes, unless it is its strip's last tab.
  const strip = workspaceOf(tabs, tab.id, ctx.filedIn);
  const another = tabs.order.some(
    (id) => id !== tab.id && workspaceOf(tabs, id, ctx.filedIn) === strip,
  );
  if (another || strip === undefined) {
    const closed = closeTab(tabs, tab.id, ctx.filedIn, ctx.pinned ?? nothingPinned);
    return { tabs: closed, moved, shows };
  }
  const pane = tabs.named.panes + 1;
  const emptied = withShows(
    {
      ...tab,
      layout: { kind: "pane", pane, content: { kind: "empty", workspace: strip } },
      focused: pane,
    },
    {},
  );
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

type At = { tab: number; pane: number };

/** Pane `from` and pane `to` trade places, each keeping its number, across tabs too. */
function swap(tabs: Tabs, from: At, to: At): Tabs {
  const one = paneAt(tabs, from);
  const other = paneAt(tabs, to);
  if (one === undefined || other === undefined) return tabs;
  const exchange = (layout: Layout): Layout =>
    layout.kind === "pane"
      ? layout.pane === one.pane
        ? other
        : layout.pane === other.pane
          ? one
          : layout
      : { ...layout, children: layout.children.map(exchange) as [Layout, Layout] };
  let next = edit(tabs, from.tab, exchange);
  if (from.tab !== to.tab) {
    next = edit(next, to.tab, exchange);
    // What each pane showed in place of its own chat goes with it.
    const shownOne = tabs.byId[from.tab].shows?.[one.pane];
    const shownOther = tabs.byId[to.tab].shows?.[other.pane];
    next = moveShows(next, from.tab, one.pane, other.pane, shownOther);
    next = moveShows(next, to.tab, other.pane, one.pane, shownOne);
    const source = next.byId[from.tab];
    if (source.focused === one.pane)
      next = { ...next, byId: { ...next.byId, [from.tab]: { ...source, focused: other.pane } } };
  }
  return inFront(next, to.tab, one.pane);
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
  const shows = showsWhere(tab, (pane) => pane !== gone);
  if (shown !== undefined) shows[came] = shown;
  return { ...tabs, byId: { ...tabs.byId, [id]: withShows(tab, shows) } };
}

/** What `tab` shows in place of its own chat, for the panes `keep` keeps. */
function showsWhere(tab: Tab, keep: (pane: number) => boolean): Record<number, number> {
  return Object.fromEntries(
    Object.entries(tab.shows ?? {}).filter(([pane]) => keep(Number(pane))),
  ) as Record<number, number>;
}

/** `tab` showing `shows`, and no `shows` at all when it is empty, as `tabs.ts` keeps a tab. */
function withShows(tab: Tab, shows: Record<number, number> | undefined): Tab {
  const next: Tab = { ...tab };
  delete next.shows;
  if (shows !== undefined && Object.keys(shows).length > 0) next.shows = shows;
  return next;
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

function paneAt(tabs: Tabs, at: At): Pane | undefined {
  const tab = tabs.byId[at.tab];
  return tab === undefined ? undefined : panes(tab.layout).find(({ pane }) => pane === at.pane);
}

const firstPane = (layout: Layout): number => panes(layout)[0].pane;

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
