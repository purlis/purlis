// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState, useSyncExternalStore } from "react";
import type { PlaneId } from "./bindings";
import type { Place } from "./pieceViews";

/**
 * **A jump to a file at a line** (FM-8, #1111; #984): one way, for every surface that names a
 * file and a line — a search hit now, a diff hunk, a session record or the knowledge graph next.
 *
 * The window brings the file's project forward and opens the branch's file tab (`Files ·
 * <branch>`), whose preview picks the file and brings the line into view. Asked through the
 * window, because a hit of "all open projects" can be another project's, and a project's tabs
 * are its own (`App.tsx` owns which project is in front).
 *
 * Two halves: {@link jumpTo} asks; the window hears it ({@link useJumpAsks}) and opens the tab;
 * the tab reads what is pending for it ({@link usePendingJump}). The pending jump is kept until
 * a newer one replaces it, so a tab that mounts after the ask still lands on the line.
 */

/**
 * Where to land: the project, the branch, the path in it, and the line and column, from 1. A
 * jump with no line opens the file in a tab of its own (a pane's file link, #6216).
 */
export type Jump = { plane: PlaneId; place: Place; path: string; line?: number; column?: number };

/** A jump with its count: each ask is answered once, even for the same place twice. */
export type Pending = Jump & { at: number };

let asked = 0;
let pending: Pending | undefined;
const tabs = new Set<() => void>();
const windows = new Set<(jump: Pending) => void>();

/** Asks the window to open `jump`'s file at its line. */
export function jumpTo(jump: Jump): void {
  const now = { ...jump, at: ++asked };
  // Only a jump to a line lands in a branch's file tab; one without opens the file's own tab.
  if (jump.line !== undefined) pending = now;
  for (const told of windows) told(now);
  for (const told of tabs) told();
}

/** The window hears each jump asked: it brings the project forward and opens the file tab. */
export function useJumpAsks(heard: (jump: Pending) => void): void {
  useEffect(() => {
    windows.add(heard);
    return () => {
      windows.delete(heard);
    };
  }, [heard]);
}

/** The jump counted `at` has landed: nothing is pending any more, unless a newer one is. */
export function settleJump(at: number): void {
  if (pending?.at === at) pending = undefined;
}

function placeKey(plane: PlaneId, place: Place): string {
  return `${plane}\u0000${place.workspace}/${place.repo}/${place.piece ?? ""}`;
}

/** The newest jump into `place` of `plane`, if the newest jump asked is one. */
export function usePendingJump(plane: PlaneId, place: Place): Pending | undefined {
  const mine = placeKey(plane, place);
  return useSyncExternalStore(
    (told) => {
      tabs.add(told);
      return () => {
        tabs.delete(told);
      };
    },
    () =>
      pending !== undefined && placeKey(pending.plane, pending.place) === mine
        ? pending
        : undefined,
  );
}

/** **A jump a tab took**: its count, the path it named, and its line and column. */
export type Landed = Pick<Pending, "at" | "path" | "line" | "column">;

/**
 * **The newest pending jump into `place` that `takes` accepts, once the tab drawing it took it**:
 * the Files tab's (FM-8) and a project-root file's own tab's (#6356) one way to land. It is taken
 * while drawing, the way React keeps what an earlier render saw, so the tab never draws the old
 * file first, and `took` runs then too, for what else the tab picks with it; it is settled after,
 * so a tab opened later does not land on it again.
 */
export function useLandedJump(
  plane: PlaneId,
  place: Place,
  takes: (jump: Pending) => boolean = () => true,
  took?: (jump: Pending) => void,
): Landed | undefined {
  const jump = usePendingJump(plane, place);
  const [landed, setLanded] = useState<Landed>();
  if (jump !== undefined && jump.at !== landed?.at && takes(jump)) {
    setLanded({ at: jump.at, path: jump.path, line: jump.line, column: jump.column });
    took?.(jump);
  }
  useEffect(() => {
    if (landed !== undefined) settleJump(landed.at);
  }, [landed]);
  return landed;
}
