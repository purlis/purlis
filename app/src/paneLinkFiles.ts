// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from "react";
import type { LinkOpened, PlaneId } from "./bindings";
import { jumpTo } from "./fileJump";
import { PLANE_ROOT, workspaceFiles, type Place } from "./pieceViews";

/**
 * **A file a pane's link opened, shown in an editor tab** (#6216, spec #6208 decision 7). The
 * core answered where the file is read from (`LinkOpened`'s `file`), and the window shows it in
 * the light editor at its line and column. It is never handed to the system's default app, so a
 * click never runs anything.
 *
 * - **A file of the project** goes the one way every surface that names a file at a line goes,
 *   a jump ({@link jumpTo}): the branch's file tab at the line, or the file's own tab.
 * - **A file outside the project** opens its own read-only tab (`pieceViews.outsideFileView`),
 *   asked of the project's view ({@link useOutsideFileAsks}). Its line and column wait here for
 *   the tab to land on ({@link outsideLanding}).
 */

/** The open command's answer for a file. */
export type FileOpened = Extract<LinkOpened, { answer: "file" }>;

/** A file outside the project, to open: in which project's tabs, and where to land. */
export type OutsideAsk = { plane: PlaneId; path: string; line?: number; column?: number };

const projects = new Set<(ask: OutsideAsk) => void>();
const landings = new Map<string, { line?: number; column?: number }>();

/** The place a file of the project is read by, or `undefined` outside it. */
export function placeOf(at: FileOpened["at"]): { place: Place; path: string } | undefined {
  switch (at.in) {
    case "branch":
      return { place: { workspace: at.workspace, repo: at.repo, piece: at.piece }, path: at.path };
    case "workspace":
      return { place: workspaceFiles(at.workspace), path: at.path };
    case "project":
      return { place: PLANE_ROOT, path: at.path };
    case "outside":
      return undefined;
  }
}

/** **Opens the file `opened` names**, in `plane`'s tabs, at its line and column. */
export function openFileLink(plane: PlaneId, opened: FileOpened): void {
  const line = opened.line ?? undefined;
  const column = opened.column ?? undefined;
  const of = placeOf(opened.at);
  if (of !== undefined) {
    jumpTo({ plane, place: of.place, path: of.path, line, column });
    return;
  }
  if (opened.at.in !== "outside") return;
  const ask = { plane, path: opened.at.path, line, column };
  landings.set(ask.path, { line, column });
  for (const told of projects) told(ask);
}

/** A project's view hears each file outside the project asked to open in its tabs. */
export function useOutsideFileAsks(plane: PlaneId, heard: (ask: OutsideAsk) => void): void {
  useEffect(() => {
    const mine = (ask: OutsideAsk) => {
      if (ask.plane === plane) heard(ask);
    };
    projects.add(mine);
    return () => {
      projects.delete(mine);
    };
  }, [plane, heard]);
}

/** Where a file outside the project's tab lands, asked once: the line and column it opened at. */
export function outsideLanding(path: string): { line?: number; column?: number } {
  const landing = landings.get(path) ?? {};
  landings.delete(path);
  return landing;
}
