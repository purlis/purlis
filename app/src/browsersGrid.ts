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
  /** What its chat calls it (`b1`), or `null` for one the person opened. */
  name: string | null;
  /** Its chat by the name it has in its project, what that chat was called for a pinned one
   *  whose chat closed (#2022), or `null` for one the person opened. */
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
let listed: readonly ProjectBrowsers[] = [];
const listeners = new Set<() => void>();

function changed(): void {
  listed = [...told.values()];
  for (const listener of listeners) listener();
}

/** Project `entry.plane` has these browsers now. */
export function tellProjectBrowsers(entry: ProjectBrowsers): void {
  if (told.get(entry.plane) === entry) return;
  told.set(entry.plane, entry);
  changed();
}

/** Forgets project `plane`'s browsers, as it closes; every project's where none is named. */
export function forgetProjectsBrowsers(plane?: string): void {
  if (plane === undefined) told.clear();
  else if (!told.delete(plane)) return;
  changed();
}

/** Every project's browsers, as they were told. */
export function projectsBrowsers(): readonly ProjectBrowsers[] {
  return listed;
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** {@link projectsBrowsers}, for a component that redraws when a project tells it again. */
export function useProjectsBrowsers(): readonly ProjectBrowsers[] {
  return useSyncExternalStore(subscribe, projectsBrowsers);
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
