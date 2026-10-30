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
 * - **A persona's removal** (`unsavedIn` with the persona, #4029): the work a save would write
 *   into the persona, a persona-level skill, which goes with it.
 * - **A project let go of** (`discardUnsavedIn`): what was kept for it goes, as its tabs do.
 *
 * **Discard and leave lets nothing go by itself**: the work goes when what holds it does: a
 * closed tab's (`discardGone`), a project let go of, a removed workspace's, the app's at a quit.
 * So a close answered with a leave and then not carried out (its own question cancelled) loses
 * nothing, and the work is still asked about the next time.
 *
 * **It is the window's, kept in memory**, as a view tab's history is (`viewHistory.ts`): a view
 * is drawn only while its tab is in front, so what it registers outlives the view's render, and
 * a relaunch starts afresh. The registry never reads a view's state itself: what it holds is
 * what the view said last.
 *
 * **It goes where its project goes** (#4028): each work says how to carry it to another window
 * (`handOver`), as plain data the window it arrives in makes the same work of again
 * (`takesHandedWork`). A project moved to another window, or handed back to the main window by
 * a split window's close, takes its work along, unasked: nothing is lost (`unsavedWindows.ts`).
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
  /** The persona a save writes into, where it is a persona's: its removal loses it too. */
  persona?: string;
  /** What it is, as the question names it (`deploy`). */
  name: string;
  /** **What kind of work it is**, for the window it is carried to (#4028): the one
   *  {@link takesHandedWork} names the kind that makes it again there. */
  kind: string;
  /** **Its state as plain data**, to carry it to another window (#4028). */
  handOver: () => unknown;
  /** Saves it, and unregisters it once saved: `undefined`, or why it was not saved. */
  save: () => Promise<string | undefined>;
  /** Lets it go, and unregisters it. */
  discard: () => void;
  /** Whether it could be saved as it is, where the view can check: `false` where a save would
   *  be refused, and the question offers no Save and leave. */
  check?: () => Promise<boolean>;
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

/** **What holds work a save writes into**, and whose removal loses it: a workspace or a
 *  persona of the project. */
export type Holder = { workspace: string } | { persona: string };

/** Whether `one` is work a save writes into `holder`. */
function heldBy(one: UnsavedWork, holder: Holder): boolean {
  return "workspace" in holder
    ? one.workspace === holder.workspace
    : one.persona === holder.persona;
}

/** The work in `plane`, or only what a save writes into `holder` where one is named. */
export function unsavedIn(plane: string, holder?: Holder): readonly UnsavedWork[] {
  return now.filter((one) => one.plane === plane && (holder === undefined || heldBy(one, holder)));
}

/** **`plane` is let go of**: what it held goes with its tabs, unasked, as they do. */
export function discardUnsavedIn(plane: string): void {
  for (const one of unsavedIn(plane)) one.discard();
  // A view that did not take itself off still goes.
  for (const one of unsavedIn(plane)) works.delete(one.key);
  told();
}

/** The view keys every tab of `tabs` shows. */
function shown(tabs: Tabs): Set<string> {
  const keys = new Set<string>();
  for (const id of tabs.order)
    for (const { content } of contentsOf(tabs, id))
      if (content.kind === "view") keys.add(viewKey(content.view));
  return keys;
}

/** **The tabs of `plane` went from `before` to `after`**: the work of each view no tab shows any
 *  more goes with its tab, whichever way it closed. */
export function discardGone(plane: string, before: Tabs, after: Tabs): void {
  if (before === after) return;
  const was = shown(before);
  const is = shown(after);
  for (const one of unsavedIn(plane)) {
    const key = viewKey(one.view);
    if (was.has(key) && !is.has(key)) one.discard();
  }
}

/** **`holder`, a workspace or a persona of `plane`, was removed**: the work a save would have
 *  written into it goes with it. */
export function discardRemoved(plane: string, holder: Holder): void {
  for (const one of unsavedIn(plane, holder)) one.discard();
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

/** **A work carried to another window** (#4028): what it is, and its state as plain data. */
export type HandedWork = {
  kind: string;
  key: string;
  plane: string;
  name: string;
  data: unknown;
};

const takers = new Map<string, (key: string, data: unknown) => void>();

/** **How a work of `kind` is made again** in the window it is carried to: `take` registers it
 *  there, under its own key, from the data its `handOver` gave. */
export function takesHandedWork(kind: string, take: (key: string, data: unknown) => void): void {
  takers.set(kind, take);
}

/** The work of `plane`, or of the whole window, as it is carried to another. */
export function handedOver(plane?: string): HandedWork[] {
  return (plane === undefined ? now : unsavedIn(plane)).map((one) => ({
    kind: one.kind,
    key: one.key,
    plane: one.plane,
    name: one.name,
    data: one.handOver(),
  }));
}

/** **Work carried here from another window**: each is registered again, by its kind's taker.
 *  A kind this window has no taker for is not one it can hold, and is dropped. */
export function takeHanded(works: readonly HandedWork[]): void {
  for (const one of works) takers.get(one.kind)?.(one.key, one.data);
}
