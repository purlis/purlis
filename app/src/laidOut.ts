// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { OUTSIDE } from "./actions";
import type { LaidNode, LaidOut, LaidTab, ViewTab } from "./bindings";
import { DRAFT, isMemory, memoryRefOf } from "./memories";
import {
  type Content,
  EMPTY_TAB,
  type Layout,
  type Tab,
  type Tabs,
  type ViewRef,
  setSplit,
  splitOf,
} from "./tabs";

/**
 * **The window's tabs, as the core holds them** (#4751): every tab as a tree of panes, told to
 * the core whenever it changes (`window_layout`), and put back from the record at a launch
 * (`reopened_layout`). The record keeps it, so a split, its sizes, its views and each strip's
 * front tab survive a restart, and the core derives what is on screen from it.
 *
 * Plain values in and out, so the whole round trip is one thing to test.
 */

/** An even split, in tenths of a percent: what a split that never moved holds. */
export const EVEN = 500;

/** The whole of a split, in tenths of a percent. */
export const WHOLE = 1000;

/** What a split holds as its first child's share: its own, or even. */
export function shareOf(layout: Extract<Layout, { kind: "split" }>): number {
  return layout.share ?? EVEN;
}

/** What {@link layoutOf} needs to know that the tabs alone do not say. */
export type Saying = {
  /** The strip tab `id` is on: a workspace, {@link OUTSIDE}, or not known yet. */
  workspaceOf: (id: number) => string | undefined;
  /** The tab last in front on `workspace`'s strip, where it is not the strip drawn. */
  frontOf: (workspace: string) => number | undefined;
  /** Whether the core has chat `session` open: a chat a scenario spec pretends is not. */
  told: (session: number) => boolean;
};

const stripName = (workspace: string | undefined): string | null =>
  workspace === undefined || workspace === OUTSIDE ? null : workspace;

/**
 * **Every tab of window `window`, as the core holds them**: each in the strip order, with its
 * strip, its pin, whether it is its strip's front tab, its focused pane's path and its tree.
 * The strip drawn is the one the tab in front is on. Each pane holds the chat it SHOWS (#1486):
 * what is on screen is derived from it.
 *
 * **Each window says its own tabs** (#4768): `0` the project window, any other number one of
 * its tab windows, which has one strip whatever workspaces its tabs are on, so its one front
 * tab is the tab in front.
 */
export function layoutOf(tabs: Tabs, say: Saying, window = 0): LaidOut {
  const strip = tabs.inFront === undefined ? null : stripName(say.workspaceOf(tabs.inFront));
  const front = (id: number, on: string | null): boolean => {
    if (window !== 0) return id === tabs.inFront;
    if (tabs.inFront !== undefined && on === strip) return id === tabs.inFront;
    const last = say.frontOf(on ?? OUTSIDE);
    return last === id;
  };
  return {
    strip,
    tabs: tabs.order.flatMap((id): LaidTab[] => {
      const tab = tabs.byId[id];
      if (tab === undefined) return [];
      const on = stripName(say.workspaceOf(id));
      return [
        {
          workspace: on,
          pinned: tab.pinned === true,
          window,
          front: front(id, on),
          focused: pathTo(tab.layout, tab.focused) ?? "",
          tree: nodeOf(tabs, tab, tab.layout, say),
        },
      ];
    }),
  };
}

function nodeOf(tabs: Tabs, tab: Tab, layout: Layout, say: Saying): LaidNode {
  if (layout.kind === "split")
    return {
      kind: "split",
      direction: layout.direction,
      share: shareOf(layout),
      a: nodeOf(tabs, tab, layout.children[0], say),
      b: nodeOf(tabs, tab, layout.children[1], say),
    };
  const content = layout.content;
  if (content.kind === "empty") return { kind: "empty" };
  if (content.kind === "session") {
    const shown = tab.shows?.[layout.pane] ?? content.session;
    return say.told(shown) ? { kind: "chat", chat: shown } : { kind: "empty" };
  }
  // A new memory's tab holds nothing on disk yet, so there is nothing to bring back.
  if (isMemory(content.view) && memoryRefOf(content.view.key)?.slug === DRAFT)
    return { kind: "empty" };
  const lead = firstPane(tab.layout) === layout.pane;
  return {
    kind: "view",
    view: {
      from: content.view.from,
      view: content.view.view,
      key: content.view.key,
      title: lead ? tab.name : content.view.view,
      workspace: stripName(content.workspace),
      at: 0,
      active: false,
      pinned: false,
      split: splitOf(tabs, content.view) ?? null,
    },
  };
}

function firstPane(layout: Layout): number {
  return layout.kind === "pane" ? layout.pane : firstPane(layout.children[0]);
}

/** The path from `layout`'s root to pane `pane`: `a` for a split's first child, `b` for its
 *  second. Nothing where the pane is not in it. */
export function pathTo(layout: Layout, pane: number): string | undefined {
  if (layout.kind === "pane") return layout.pane === pane ? "" : undefined;
  const [a, b] = layout.children;
  const inA = pathTo(a, pane);
  if (inA !== undefined) return `a${inA}`;
  const inB = pathTo(b, pane);
  return inB === undefined ? undefined : `b${inB}`;
}

/**
 * The split at `path` in tab `id` holds `share` of itself now: a person dragged its handle.
 * Answers `tabs` itself when nothing changed, or when there is no split there.
 */
export function withShare(tabs: Tabs, id: number, path: string, share: number): Tabs {
  const tab = tabs.byId[id];
  if (tab === undefined) return tabs;
  const held = Math.round(Math.min(WHOLE, Math.max(0, share)));
  const at = (layout: Layout, rest: string): Layout => {
    if (layout.kind !== "split") return layout;
    if (rest === "") return shareOf(layout) === held ? layout : { ...layout, share: held };
    const side = rest[0] === "b" ? 1 : 0;
    const child = at(layout.children[side], rest.slice(1));
    if (child === layout.children[side]) return layout;
    const children: [Layout, Layout] =
      side === 0 ? [child, layout.children[1]] : [layout.children[0], child];
    return { ...layout, children };
  };
  const layout = at(tab.layout, path);
  if (layout === tab.layout) return tabs;
  return { ...tabs, byId: { ...tabs.byId, [id]: { ...tab, layout } } };
}

/** What {@link tabsOf} needs to know of what came back at a launch. */
export type Reopening = {
  /** Chat `session`'s names, where it came back and is drawn; nothing for one that did not. */
  chat: (session: number) => { name: string; who: string | null; label: string | null } | undefined;
  /** A recorded view as it is drawn now: renamed, or a studio's put back as the one studio. */
  view: (view: ViewRef, title: string) => { view: ViewRef; title: string };
};

/** What a launch puts back from the record's tabs. */
export type PutBack = {
  tabs: Tabs;
  /** Every chat a pane put back, so the rest get the tabs of their own they always had. */
  placed: Set<number>;
  /** Each strip's front tab, by workspace ({@link OUTSIDE} for the strip outside them). */
  fronts: Record<string, number>;
  /** The tab in front of the strip drawn, where one came back. */
  inFront?: number;
};

/**
 * **The record's tabs of window `window`, as that window draws them** (#4751, #4768), added
 * to `tabs`. A chat another window shows is placed there, and gets no tab here. A pane whose chat
 * did not come back and a chat already in an earlier pane are left out, and the split that held
 * one gives its place to the other side; an empty pane comes back empty (#4755). A tab left with
 * nothing is not put
 * back. Each view waits for a press, as a view tab put back always has. The tab in front is
 * left as `tabs` had it: {@link PutBack.inFront} says which to bring forward.
 */
export function tabsOf(laid: LaidOut, tabs: Tabs, reopening: Reopening, window = 0): PutBack {
  let next = tabs;
  // **Another window's chats are placed there** (#4768): they get no tab of their own here.
  const placed = new Set<number>(
    laid.tabs.filter((one) => one.window !== window).flatMap((one) => chatsOf(one.tree)),
  );
  const fronts: Record<string, number> = {};
  let inFront: number | undefined;
  for (const one of laid.tabs.filter((each) => each.window === window)) {
    let panes = next.named.panes;
    const workspace = one.workspace ?? OUTSIDE;
    const byPath = new Map<string, number>();
    const titles = new Map<number, string>();
    const splits: [ViewRef, number][] = [];
    const drawn = (node: LaidNode, path: string): Layout | undefined => {
      if (node.kind === "split") {
        const a = drawn(node.a, `${path}a`);
        const b = drawn(node.b, `${path}b`);
        if (a === undefined) return b;
        if (b === undefined) return a;
        return {
          kind: "split",
          direction: node.direction,
          share: Math.min(WHOLE, Math.max(0, node.share)),
          children: [a, b],
        };
      }
      let content: Content;
      // **An empty pane comes back as one** (#4755): the person's, kept in its place.
      if (node.kind === "empty") content = { kind: "empty", workspace };
      else if (node.kind === "chat") {
        const chat = reopening.chat(node.chat);
        if (chat === undefined || placed.has(node.chat)) return undefined;
        placed.add(node.chat);
        // **A task comes back as itself, in its pane** (#4757): a press put it there, as any
        // chat, and no tab is its asker's to be drawn into.
        content = { kind: "session", session: node.chat, chat: chat.name };
      } else {
        const now = reopening.view(refOf(node.view), node.view.title);
        content = { kind: "view", view: now.view, workspace, waits: true };
        titles.set(panes + 1, now.title);
        if (node.view.split !== null && node.view.split !== undefined)
          splits.push([now.view, node.view.split]);
      }
      panes += 1;
      byPath.set(path, panes);
      return { kind: "pane", pane: panes, content };
    };
    const layout = drawn(one.tree, "");
    if (layout === undefined) continue;
    const lead = firstPane(layout);
    const content = contentAt(layout, lead);
    const id = next.named.tabs + 1;
    const name =
      content.kind === "view"
        ? {
            name: titles.get(lead) ?? content.view.view,
            defaultName: titles.get(lead) ?? content.view.view,
          }
        : content.kind === "empty"
          ? { name: EMPTY_TAB, defaultName: EMPTY_TAB }
          : chatName(content.session, reopening);
    const focused = byPath.get(one.focused) ?? lead;
    next = {
      ...next,
      byId: {
        ...next.byId,
        [id]: {
          id,
          name: name.name,
          defaultName: name.defaultName,
          layout,
          focused,
          // **The pin is the tab's** (#4935), whatever its first pane holds.
          ...(one.pinned ? { pinned: true as const } : {}),
          // **The tab's own strip** (#4757), as the record says it: a pane may show a chat of
          // another workspace, and the tab stays where it was.
          workspace,
        },
      },
      order: [...next.order, id],
      named: { tabs: id, panes },
    };
    for (const [view, split] of splits) next = setSplit(next, view, split);
    if (one.front) fronts[workspace] = id;
    if (one.front && window === 0 && one.workspace === laid.strip) inFront = id;
    // A tab window shows its first front tab, whatever strip it is on.
    if (one.front && window !== 0 && inFront === undefined) inFront = id;
  }
  return { tabs: next, placed, fronts, inFront };
}

/** Every chat in a pane of `node`, in reading order. */
export function chatsOf(node: LaidNode): number[] {
  if (node.kind === "split") return [...chatsOf(node.a), ...chatsOf(node.b)];
  return node.kind === "chat" ? [node.chat] : [];
}

function refOf(view: ViewTab): ViewRef {
  return { from: view.from, view: view.view, key: view.key };
}

function contentAt(layout: Layout, pane: number): Content {
  if (layout.kind === "pane") return layout.content;
  const [a, b] = layout.children;
  return pathTo(a, pane) !== undefined ? contentAt(a, pane) : contentAt(b, pane);
}

/** What a tab put back on chat `session` is called: what a tab opened on it is called. */
function chatName(session: number, reopening: Reopening): { name: string; defaultName: string } {
  const chat = reopening.chat(session);
  const plain = chat?.name ?? String(session);
  const defaultName = chat?.who ? `${chat.who} ${plain}` : plain;
  return { name: chat?.label ?? defaultName, defaultName };
}
