// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Tab, Tabs } from "./tabs";

/**
 * **Undo after a layout change** (#4763, #4765; #4583: "Undo after every layout change, ⌘Z"):
 * one step for the whole window, held on the tabs (`Tabs.undo`). A drop, a split and Tile all
 * each record theirs with {@link recorded}; the toast, ⌘Z and the palette's row all put back
 * the same one ({@link undoLayout}). It holds only while the tabs are as the change left them:
 * a later change, in any tab, retires it.
 */

/** `after`, a change made to `before`, with Undo holding `before` and what it `did`. A change
 *  that changed nothing records nothing and answers `before` itself. */
export function recorded(before: Tabs, after: Tabs, did: string): Tabs {
  if (after === before) return before;
  const was = { ...before };
  delete was.undo;
  return { ...after, undo: { before: was, after, did } };
}

/** Whether Undo would put the last layout change back: the tabs are still as it left them. */
export function canUndoLayout(tabs: Tabs): boolean {
  return tabs.undo !== undefined && undone(tabs, tabs.undo.before, tabs.undo.after) !== undefined;
}

/** **Undo**: the tabs before the last layout change, once. Answers `tabs` itself when there is
 *  nothing to undo. */
export function undoLayout(tabs: Tabs): Tabs {
  const back = tabs.undo && undone(tabs, tabs.undo.before, tabs.undo.after);
  if (back === undefined) return tabs;
  delete back.undo;
  return back;
}

/**
 * **The tabs an Undo puts back**, or nothing once the tabs have moved on from `after`: what was
 * dragged since, or a chat opened or closed, is not taken back by an Undo it never saw. Focus
 * and the front tab moving are no such change.
 */
export function undone(now: Tabs, before: Tabs, after: Tabs): Tabs | undefined {
  const same =
    now.order.length === after.order.length &&
    now.order.every(
      (id, at) => after.order[at] === id && now.byId[id]?.layout === after.byId[id]?.layout,
    );
  if (!same) return undefined;
  // Only the layout goes back: a tab renamed since keeps its new name. A tab the change closed
  // comes back as it was. A zoom on a tab whose tree goes back ends with it.
  const byId = Object.fromEntries(
    before.order.map((id) => {
      const was = before.byId[id];
      const is = now.byId[id];
      if (is === undefined) return [id, was];
      const back: Tab = { ...is, layout: was.layout, focused: was.focused };
      // A tab no one has named since is named for what it shows: a close that left the strip's
      // last tab empty named it so, and its Undo names it as it was (#4756).
      if (is.name === is.defaultName) {
        back.name = was.name;
        back.defaultName = was.defaultName;
      }
      delete back.shows;
      if (was.shows !== undefined) back.shows = was.shows;
      if (is.layout !== was.layout) delete back.zoomed;
      return [id, back];
    }),
  );
  // Numbers handed out since stay handed out: a pane or a tab never reuses one.
  return { ...now, byId, order: before.order, inFront: before.inFront, named: now.named };
}
