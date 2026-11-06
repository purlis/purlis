// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { emitTo } from "@tauri-apps/api/event";
import { commands, type LaidNode, type LaidOut, type PlaneId } from "./bindings";
import { chatsOf, type Reopening, tabsOf } from "./laidOut";
import type { StartNotes } from "./ChatNotices";
import type { HeldBlock } from "./sandboxBlocks";
import {
  type Content,
  type FiledIn,
  type Layout,
  type Tabs,
  closePane,
  closeTab,
  contentsOf,
  viewKey,
} from "./tabs";

/**
 * **Tab windows, as one window sees them** (#4768, ruled on #4701): a tab torn off into a bare
 * window of its project, holding a strip and panes but no sidebar. Which window holds which
 * tab is the core's (`move_to_window`), and every window of the project is told what the core
 * holds after a move (`layout-moved`); each draws its own share of it here.
 *
 * Plain values in and out, so a move between windows is one thing to test.
 */

/** The event the core sends a project's every window after tabs moved between them. */
export const LAYOUT_MOVED = "layout-moved";

/** **The event a tab window's own report sends every window of the project** (#5225): the same
 *  layout, for what each window shows. Nothing moved, so no window draws anything from it. */
export const LAYOUT_REPORTED = "layout-reported";

/** The event a window sends every window as a note of a chat is cleared in it (#5225): each may
 *  hold the note, for the chat coming back to it, and none says it again. */
export const NOTE_CLEARED = "note-cleared";

/** What {@link NOTE_CLEARED} carries: which chat's note, by the state that holds it. */
export type NoteCleared = {
  plane: string;
  session: number;
  note: { kind: "start" | "by-hand" | "restart" } | { kind: "block"; block: HeldBlock };
  /** The window it was cleared in, which hears it too. */
  from: string;
};

/** **The event a window sends as it comes to hold notes of a chat another window shows** (#5225):
 *  that window may have heard none of them, having shown the chat only since. */
export const NOTES_HANDED = "notes-handed";

/** What {@link NOTES_HANDED} carries: the notes a window holds in its own state. */
export type NotesHanded = {
  plane: string;
  session: number;
  start?: StartNotes;
  blocks: readonly HeldBlock[];
  /** The window that holds them, which hears it too. */
  from: string;
};

/** **The event a tab window sends as it first draws** (#5225): it was not listening as its chats
 *  moved to it, so each window hands over again what it holds of them. */
export const NOTES_ASKED = "notes-asked";

/** What {@link NOTES_ASKED} carries. */
export type NotesAsked = { plane: string; from: string };

/** `held`, with what was handed over added: each line and each file once. */
export function withStartHanded(held: StartNotes | undefined, handed: StartNotes): StartNotes {
  if (held === undefined) return handed;
  return {
    notes: [...new Set([...held.notes, ...handed.notes])],
    agentsMd: held.agentsMd.length > 0 ? held.agentsMd : handed.agentsMd,
  };
}

/** The event a window is sent to show a chat: a press in another window of the project landed
 *  on it (`press_elsewhere`), on a chat it has in a pane, or on a chat in no pane where its
 *  pane was used last (#5108), which it shows in its focused pane. A tab window sends it to the
 *  project window for a chat of another strip ({@link pressInProjectWindow}, #5330). */
export const PRESS_HERE = "press-here";

/** What {@link PRESS_HERE} carries: the core's `tabwindows::PressHere`. */
export type PressHere = { plane: string; chat: number };

/**
 * **A press a tab window does not take goes to the project window** (#5330): the window holding
 * `plane` comes forward and is sent {@link PRESS_HERE}, so it opens chat `chat` by the rule
 * there (#5289). Answers whether any window holds the project.
 */
export async function pressInProjectWindow(plane: PlaneId, chat: number): Promise<boolean> {
  const label = await commands.showWindowHolding(plane).catch(() => null);
  if (label === null) return false;
  await emitTo(label, PRESS_HERE, { plane, chat } satisfies PressHere).catch(() => undefined);
  return true;
}

/** What {@link LAYOUT_MOVED} carries. */
export type LayoutMoved = { plane: string; layout: LaidOut };

/** What a tab shows, by identity: each chat by its number, each view by its key. Empty panes
 *  are nothing, so a tab of only empty panes is never said to have moved. */
function shownBy(layout: Layout, shows?: Readonly<Record<number, number>>): string[] {
  if (layout.kind === "split")
    return [...shownBy(layout.children[0], shows), ...shownBy(layout.children[1], shows)];
  return paneShows(layout.content, shows?.[layout.pane]);
}

/** What one pane shows, by identity: nothing for an empty pane. */
function paneShows(content: Content, shown?: number): string[] {
  if (content.kind === "session") return [`chat:${shown ?? content.session}`];
  if (content.kind === "view") return [`view:${viewKey(content.view)}`];
  return [];
}

function laidShows(node: LaidNode): string[] {
  if (node.kind === "split") return [...laidShows(node.a), ...laidShows(node.b)];
  if (node.kind === "chat") return [`chat:${node.chat}`];
  if (node.kind === "view")
    return [`view:${viewKey({ from: node.view.from, view: node.view.view, key: node.view.key })}`];
  return [];
}

/**
 * **The chats the project's other windows show** (#5225), as a layout says it: what `window` has
 * to say of those is theirs to say, on their panes.
 */
export const chatsElsewhere = (laid: LaidOut, window: number): ReadonlySet<number> =>
  new Set(laid.tabs.filter((one) => one.window !== window).flatMap((one) => chatsOf(one.tree)));

/**
 * **What window `window` holds once the core moved tabs between the project's windows**: each
 * of its tabs the core now has in another window leaves (the tab beside it coming forward, as
 * a close sends it), each pane it has there leaves its split to the pane beside it, and each tab the core has in this window that this window does not draw
 * arrives. One the core says is in front comes forward (Move to window); one that is not goes
 * just behind the tab in front, and nothing it holds comes forward (a closed tab window's tabs
 * sent back). Answers `tabs` itself, unchanged, when nothing moved.
 */
export function movedBetweenWindows(
  laid: LaidOut,
  tabs: Tabs,
  window: number,
  reopening: Reopening,
  filedIn: FiledIn,
): { tabs: Tabs; changed: boolean } {
  const elsewhere = new Set(
    laid.tabs.filter((one) => one.window !== window).flatMap((one) => laidShows(one.tree)),
  );
  const leaving = tabs.order.filter((id) => {
    const tab = tabs.byId[id];
    const shows = tab === undefined ? [] : shownBy(tab.layout, tab.shows);
    return shows.length > 0 && shows.every((one) => elsewhere.has(one));
  });
  let next = leaving.reduce((now, id) => closeTab(now, id, filedIn), tabs);
  // A pane moved out of a split (`move_pane`) leaves it to the pane beside it.
  let panesLeft = 0;
  for (const id of next.order) {
    const shows = next.byId[id]?.shows;
    for (const { pane, content } of contentsOf(next, id)) {
      const what = paneShows(content, shows?.[pane]);
      if (what.length > 0 && what.every((one) => elsewhere.has(one))) {
        next = closePane(next, id, pane, filedIn);
        panesLeft += 1;
      }
    }
  }
  const here = new Set(
    next.order.flatMap((id) => {
      const tab = next.byId[id];
      return tab === undefined ? [] : shownBy(tab.layout, tab.shows);
    }),
  );
  const arriving = laid.tabs.filter((one) => {
    if (one.window !== window) return false;
    const shows = laidShows(one.tree);
    return shows.length > 0 && !shows.some((each) => here.has(each));
  });
  if (leaving.length === 0 && panesLeft === 0 && arriving.length === 0)
    return { tabs, changed: false };
  for (const one of arriving) {
    const before = next;
    const back = tabsOf({ strip: laid.strip, tabs: [one] }, before, reopening, window);
    const id = back.tabs.order.find((each) => !before.order.includes(each));
    if (id === undefined) continue;
    if (one.front || before.inFront === undefined) {
      next = { ...back.tabs, inFront: id };
      continue;
    }
    // Behind the tab in front: just after it, and after any sent back before this one.
    const order = back.tabs.order.filter((each) => each !== id);
    const front = order.indexOf(before.inFront);
    let at = front + 1;
    while (at < order.length && !tabs.order.includes(order[at]) && order[at] !== id) at += 1;
    order.splice(at, 0, id);
    next = { ...back.tabs, order, inFront: before.inFront };
  }
  return { tabs: next, changed: true };
}

/**
 * **Whether a tab dragged along the strip was torn off** (#4768, ruled on #4701): let go with the
 * pointer past an edge of the window. A drop inside the window, its edge included, is a move
 * along the strip as it always was; the keyboard's way to the same is Move to new window. A
 * pointer never seen is no tear-off.
 */
export function tornOff(
  point: { x: number; y: number } | undefined,
  window: { width: number; height: number },
): boolean {
  if (point === undefined) return false;
  return point.x < 0 || point.y < 0 || point.x > window.width || point.y > window.height;
}
