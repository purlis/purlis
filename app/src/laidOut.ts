// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { OUTSIDE } from "./actions";
import type { LaidNode, LaidOut, LaidTab, ViewTab } from "./bindings";
import { DRAFT, isMemory, memoryRefOf } from "./memories";
import {
  type Content,
  type Layout,
  type Tab,
  type Tabs,
  type ViewRef,
  setSplit,
  splitOf,
  viewKey,
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
  /** Whether tab `id` is pinned. */
  pinned: (id: number) => boolean;
  /** The tab last in front on `workspace`'s strip, where it is not the strip drawn. */
  frontOf: (workspace: string) => number | undefined;
  /** Whether the core has chat `session` open: a chat a scenario spec pretends is not. */
  told: (session: number) => boolean;
};

const stripName = (workspace: string | undefined): string | null =>
  workspace === undefined || workspace === OUTSIDE ? null : workspace;

/**
 * **Every tab, as the core holds them**: each in the strip order, with its strip, its pin,
 * whether it is its strip's front tab, its focused pane's path and its tree. The strip drawn is
 * the one the tab in front is on. Each pane holds the chat it SHOWS (#1486): what is on screen
 * is derived from it. Every tab is the project window's until a tab can have one of its own.
 */
export function layoutOf(tabs: Tabs, say: Saying): LaidOut {
  const strip = tabs.inFront === undefined ? null : stripName(say.workspaceOf(tabs.inFront));
  const front = (id: number, on: string | null): boolean => {
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
          pinned: say.pinned(id),
          window: 0,
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
  /**
   * Chat `session`'s names, where it came back and is drawn; nothing for one that did not.
   * `drawnIn` is the chat whose tab a task with no tab of its own is drawn in (#1486): its
   * nearest asker that has one.
   */
  chat: (
    session: number,
  ) => { name: string; who: string | null; label: string | null; drawnIn?: number } | undefined;
  /** A recorded view as it is drawn now: renamed, or a studio's put back as the one studio. */
  view: (view: ViewRef, title: string) => { view: ViewRef; title: string };
};

/** What a launch puts back from the record's tabs. */
export type PutBack = {
  tabs: Tabs;
  /** Every chat a pane put back, so the rest get the tabs of their own they always had. */
  placed: Set<number>;
  /** The views of pinned tabs that open on a view, as {@link viewKey}s. */
  pinnedViews: string[];
  /** Each strip's front tab, by workspace ({@link OUTSIDE} for the strip outside them). */
  fronts: Record<string, number>;
  /** The tab in front of the strip drawn, where one came back. */
  inFront?: number;
};

/**
 * **The record's tabs, as the window draws them** (#4751), added to `tabs`. A pane whose chat
 * did not come back, a chat already in an earlier pane, and an empty pane are left out, and the
 * split that held one gives its place to the other side; a tab left with nothing is not put
 * back. Each view waits for a press, as a view tab put back always has. The tab in front is
 * left as `tabs` had it: {@link PutBack.inFront} says which to bring forward.
 */
export function tabsOf(laid: LaidOut, tabs: Tabs, reopening: Reopening): PutBack {
  let next = tabs;
  const placed = new Set<number>();
  const pinnedViews: string[] = [];
  const fronts: Record<string, number> = {};
  let inFront: number | undefined;
  const inLayout = new Set(laid.tabs.flatMap((one) => chatsOf(one.tree)));
  for (const one of laid.tabs) {
    let panes = next.named.panes;
    const workspace = one.workspace ?? OUTSIDE;
    const byPath = new Map<string, number>();
    const titles = new Map<number, string>();
    const splits: [ViewRef, number][] = [];
    const shows: Record<number, number> = {};
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
      if (node.kind === "empty") return undefined;
      if (node.kind === "chat") {
        const chat = reopening.chat(node.chat);
        if (chat === undefined || placed.has(node.chat)) return undefined;
        placed.add(node.chat);
        // **A task with no tab of its own, in a pane its asker is in no pane of, is that
        // asker's tab switched to it** (#1486), as the window has always drawn one: the tab is
        // the asker's, and its pane shows the task.
        const owner = chat.drawnIn;
        const asker = owner === undefined ? undefined : reopening.chat(owner);
        if (owner !== undefined && owner !== node.chat && asker !== undefined)
          if (!inLayout.has(owner) && !placed.has(owner)) {
            placed.add(owner);
            shows[panes + 1] = node.chat;
            content = { kind: "session", session: owner, chat: asker.name };
          } else content = { kind: "session", session: node.chat, chat: chat.name };
        else content = { kind: "session", session: node.chat, chat: chat.name };
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
          ...(Object.keys(shows).length > 0 ? { shows } : {}),
        },
      },
      order: [...next.order, id],
      named: { tabs: id, panes },
    };
    for (const [view, split] of splits) next = setSplit(next, view, split);
    if (one.pinned && content.kind === "view") pinnedViews.push(viewKey(content.view));
    if (one.front) fronts[workspace] = id;
    if (one.front && one.window === 0 && one.workspace === laid.strip) inFront = id;
  }
  return { tabs: next, placed, pinnedViews, fronts, inFront };
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
