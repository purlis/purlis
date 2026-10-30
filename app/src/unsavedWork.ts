// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A view's unsaved work, and every close that would lose it** (#3929).
 *
 * One registry for the whole window: a view that holds work not saved yet (the Skill studio's
 * editor, and any editor in a view tab after it) says so here, by a key of its own, with what to
 * call it, how to save it and how to let it go. Every way a close can lose that work asks here
 * first, and asks the one question (`UnsavedWorkQuestion`): Keep editing, Discard and leave, or
 * Save and leave.
 *
 * - **A tab's or a pane's close** (`unsavedClosedBy`): the work in the views it closes, asked in
 *   `PlaneView`'s one close path, and counted there as a close question (`closeQuestion.ts`).
 * - **A project's close, and a quit** (`unsavedIn`): every work in the project, or in the window,
 *   asked in `App` before what the close ends is.
 * - **A workspace's removal** (`unsavedIn` with the workspace): the work a save would write into
 *   it, which goes with it. Its view tabs stay (`refileViews`), so nothing else is lost.
 * - **A project let go of** (`discardUnsavedIn`): what was kept for it goes, as its tabs do.
 *
 * **It is the window's, kept in memory**, as a view tab's history is (`viewHistory.ts`): a view
 * is drawn only while its tab is in front, so what it registers outlives the view's render, and
 * a relaunch starts afresh. The registry never reads a view's state itself: what it holds is
 * what the view said last.
 */
import { useSyncExternalStore } from "react";
import type { Offer } from "./actions";
import { contentsOf, viewKey, type Tabs, type ViewRef } from "./tabs";

/** **One view's work not saved yet**, as the view registered it. */
export type UnsavedWork = {
  /** The view's own key for it: one work per key. */
  key: string;
  /** The project it is in. */
  plane: string;
  /** The view that holds it: a close of the tab or pane showing this view loses it. */
  view: ViewRef;
  /** The workspace a save writes into, where it is a workspace's: its removal loses it too. */
  workspace?: string;
  /** What it is, as the question names it (`deploy`). */
  name: string;
  /** Saves it, and unregisters it once saved: `undefined`, or why it was not saved. */
  save: () => Promise<string | undefined>;
  /** Lets it go, and unregisters it. */
  discard: () => void;
};

const works = new Map<string, UnsavedWork>();
/** What `useUnsavedWork` reads: a new list on every change, the same one between. */
let now: readonly UnsavedWork[] = [];
const listeners = new Set<() => void>();

function told(): void {
  now = [...works.values()];
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** **Registers `work` under `key`**, or takes it off (`undefined`) once it is saved or let go. */
export function keepUnsaved(key: string, work: UnsavedWork | undefined): void {
  if (work === undefined ? !works.delete(key) : works.get(key) === work) return;
  if (work !== undefined) works.set(key, work);
  told();
}

/** Every work not saved, as it is this moment. */
export function unsavedNow(): readonly UnsavedWork[] {
  return now;
}

/** Every work not saved, drawn again as it changes. */
export function useUnsavedWork(): readonly UnsavedWork[] {
  return useSyncExternalStore(subscribe, unsavedNow);
}

/** The work in `plane`, or in its workspace `workspace` where one is named. */
export function unsavedIn(plane: string, workspace?: string): readonly UnsavedWork[] {
  return now.filter(
    (one) => one.plane === plane && (workspace === undefined || one.workspace === workspace),
  );
}

/** **`plane` is let go of**: what it held goes with its tabs, unasked, as they do. */
export function discardUnsavedIn(plane: string): void {
  for (const one of unsavedIn(plane)) one.discard();
  // A view that did not take itself off still goes.
  for (const one of unsavedIn(plane)) works.delete(one.key);
  told();
}

/**
 * **The work a close would lose**, and the window tab it is in: a tab's close (`closeTab`) loses
 * the work of every view in it, a pane's (`closePane`) that of the focused pane in the tab in
 * front. `undefined` where it loses none, or the verb closes no view.
 */
export function unsavedClosedBy(
  plane: string,
  tabs: Tabs,
  does: Offer["does"],
): { work: readonly UnsavedWork[]; tab: number } | undefined {
  const tab =
    does.verb === "closeTab" ? does.tab : does.verb === "closePane" ? tabs.inFront : undefined;
  if (tab === undefined) return undefined;
  const focused = tabs.byId[tab]?.focused;
  const closing = new Set<string>();
  for (const { pane, content } of contentsOf(tabs, tab)) {
    if (does.verb === "closePane" && pane !== focused) continue;
    if (content.kind === "view") closing.add(viewKey(content.view));
  }
  const work = unsavedIn(plane).filter((one) => closing.has(viewKey(one.view)));
  return work.length === 0 ? undefined : { work, tab };
}
