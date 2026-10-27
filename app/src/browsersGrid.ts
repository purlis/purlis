// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";
import type { BrowserDriver } from "./bindings";
import { onProjectViews } from "./projectViews";
import { keepScopeOf, keptScopeOf, type InboxScope, type ScopeCounts } from "./inboxScope";
import type { ViewRef } from "./tabs";

/**
 * **The Browsers grid** (#2027, spec #2008, second version): every browser in scope as a live
 * tile, about a frame a second, in a view tab of its own. Its scope switch is the Inbox's (all
 * projects, this project, this workspace), kept per project as the Inbox's is, under the
 * project's `browsers.scope` in `layout.json`.
 *
 * A browser is its project's to know: each project this window holds tells the grid its own
 * ({@link tellProjectBrowsers}), its chats' browsers by the names they have there and the
 * person's own that its tabs show, so the grid of one project can draw another's. A press on a
 * tile is handed back to its project ({@link ProjectBrowsers.open}), which comes to the front
 * and opens the browser's view on its own strip.
 */

/** The Browsers grid's tab: one per project. */
export const BROWSERS_GRID_VIEW: ViewRef = { from: null, view: "browsers", key: "" };

/** What the Browsers grid's tab is called. */
export const BROWSERS_GRID_TITLE = "Browsers";

/** Whether `view` is the Browsers grid's. */
export function isBrowsersGrid(view: ViewRef): boolean {
  return view.from === null && view.view === BROWSERS_GRID_VIEW.view;
}

/** One browser, as the grid draws it. */
export type GridTile = {
  /** The project it is of. */
  plane: string;
  /** Its program in the session host: what its view is watched by. */
  program: string;
  /** The person opened it: no chat's, of any project this window holds. */
  own: boolean;
  /** What its chat calls it (`b1`), or `null` for one the person opened. */
  name: string | null;
  /** Its chat by the name it has in its project, what that chat was called for a pinned one
   *  whose chat closed (#2022), or `null` for one the person opened or a chat with no name
   *  kept. */
  chat: string | null;
  /** The workspace it is filed in, by the strip's key: its chat's, or its tab's. */
  workspace: string;
  /** Who drives it, where its agent does not (#2300). */
  driver: BrowserDriver | null;
  /** The pool whose runner it runs on, or `null` on this machine (#2050). */
  pool: string | null;
};

/** What one project tells the grid. */
export type ProjectBrowsers = {
  plane: string;
  /** What the project is called, as its tab says it. */
  project: string;
  tiles: readonly GridTile[];
  /** Brings the project to the front and opens `tile`'s view on its strip. */
  open: (tile: GridTile) => void;
};

/** What each project told last, in the order they first told it. */
const told = new Map<string, ProjectBrowsers>();
/** Each project's way to open one of its browsers, the latest it told. */
const opens = new Map<string, (tile: GridTile) => void>();
let listed: readonly ProjectBrowsers[] = [];
const listeners = new Set<() => void>();

/**
 * **What every project told, as the grid lists it.** A browser is the person's own only where
 * no project this window holds lists it as a chat's: a tab of one project that shows another
 * project's chat's browser does not make that browser the person's.
 */
function changed(): void {
  const ofChats = new Set(
    [...told.values()]
      .flatMap((one) => one.tiles.filter((tile) => !tile.own))
      .map((tile) => tile.program),
  );
  listed = [...told.values()].map((one) =>
    one.tiles.some((tile) => tile.own && ofChats.has(tile.program))
      ? { ...one, tiles: one.tiles.filter((tile) => !(tile.own && ofChats.has(tile.program))) }
      : one,
  );
  for (const listener of listeners) listener();
}

const sameTile = (one: GridTile, two: GridTile): boolean =>
  one.plane === two.plane &&
  one.program === two.program &&
  one.own === two.own &&
  one.name === two.name &&
  one.chat === two.chat &&
  one.workspace === two.workspace &&
  one.driver === two.driver &&
  one.pool === two.pool;

/**
 * Project `entry.plane` has these browsers now. Nobody is told when they are the ones it told
 * last, as a change to its tabs that shows no other browser leaves them: only its way to open
 * one is taken.
 */
export function tellProjectBrowsers(entry: ProjectBrowsers): void {
  opens.set(entry.plane, entry.open);
  const was = told.get(entry.plane);
  if (
    was !== undefined &&
    was.project === entry.project &&
    was.tiles.length === entry.tiles.length &&
    was.tiles.every((tile, at) => sameTile(tile, entry.tiles[at]))
  )
    return;
  told.set(entry.plane, entry);
  changed();
}

/** Forgets project `plane`'s browsers, as it closes; every project's where none is named. */
export function forgetProjectsBrowsers(plane?: string): void {
  if (plane === undefined) {
    told.clear();
    opens.clear();
  } else {
    opens.delete(plane);
    if (!told.delete(plane)) return;
  }
  changed();
}

/** **Opens `tile`'s view in its own project**, by the way that project told last. */
export function openTile(tile: GridTile): void {
  opens.get(tile.plane)?.(tile);
}

/** Every project's browsers, as they were told. */
export function projectsBrowsers(): readonly ProjectBrowsers[] {
  return listed;
}

/** Calls `listener` each time a project tells the grid something new. Answers how to stop. */
export function onProjectsBrowsers(listener: () => void): () => void {
  listeners.add(listener);
  return () => void listeners.delete(listener);
}

/** {@link projectsBrowsers}, for a component that redraws when a project tells it again. */
export function useProjectsBrowsers(): readonly ProjectBrowsers[] {
  return useSyncExternalStore(onProjectsBrowsers, projectsBrowsers);
}

/**
 * **The browsers `scope` lists**, seen from project `plane` with workspace `here` in front:
 * every project's, the project's, or those filed in that workspace of the project.
 */
export function tilesIn(
  scope: InboxScope,
  projects: readonly ProjectBrowsers[],
  plane: string,
  here: string | undefined,
): GridTile[] {
  return projects
    .filter((one) => scope === "all" || one.plane === plane)
    .flatMap((one) => one.tiles)
    .filter((tile) => scope !== "workspace" || tile.workspace === here);
}

/** How many browsers each scope lists; the workspace's only where one is in front. */
export function gridCounts(
  projects: readonly ProjectBrowsers[],
  plane: string,
  here: string | undefined,
): ScopeCounts {
  return {
    all: tilesIn("all", projects, plane, here).length,
    project: tilesIn("project", projects, plane, here).length,
    ...(here !== undefined
      ? { workspace: tilesIn("workspace", projects, plane, here).length }
      : {}),
  };
}

/** The scope the person last picked for the grid of `plane`, or the project's. */
export function keptBrowsersScope(plane: string | undefined): InboxScope {
  return keptScopeOf(plane, "browsers");
}

/** Keeps `scope` as the grid's for `plane`. The default is kept as nothing. */
export function keepBrowsersScope(plane: string | undefined, scope: InboxScope): void {
  keepScopeOf(plane, "browsers", scope);
}

/** {@link keptBrowsersScope}, for a component that redraws when it is picked again. */
export function useBrowsersScope(plane: string | undefined): InboxScope {
  return useSyncExternalStore(onProjectViews, () => keptBrowsersScope(plane));
}
