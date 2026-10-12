// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Panels as PanelsModel, Piece, RepoStates } from "./bindings";

/**
 * Everything three regions need to know about ONE workspace, asked once.
 *
 * **Asked here and not in each region, because three of them now want the same answer.**
 * Until ADR 0038 the right-hand `Panels` asked for itself, and its own comment argued that
 * the asks did not belong further up. That was right while it was the only region reading a
 * workspace. It is wrong now: the explorer on the left, the state bar at the bottom and the
 * right-hand side all draw the same workspace, and three copies of these asks would be
 * `workspace_panels` three times, `workspace_repos` three times — and `workspace_repos` runs
 * `git status` once per clone, bounded at five seconds each.
 *
 * **What it costs, counted rather than assumed** (the standard #133 set). Per focused
 * workspace, not per render and not per keystroke:
 *
 * - one `workspace_panels` — a directory listing and a few small files;
 * - one `workspace_repos` — `git status` per clone, on a blocking thread in the core;
 * - one `worktree_list` **per clone** — `git worktree list --porcelain` per clone, which is
 *   the ask this record adds. It is the explorer's whole content, so it cannot be avoided;
 *   what it can be is bounded, and it is: these calls are made when the clone NAMES change,
 *   which they do not for as long as the plane answers the same way, so focusing a workspace
 *   asks once and re-rendering asks not at all. That bound is pinned by a test rather than
 *   described — `workspacePanels.test.tsx`, "asks git for the pieces once per clone". Inside that one ask, a clone with any piece on a branch also costs one
 *   `git config --get-regexp` (which branches purlis cut) and a read of the workspace's piece
 *   log, so a row can say a branch is unclaimed (#835); a clone with no such piece costs neither.
 *
 * **Asked by `workspacePanels.ts`** (#1160): a store outside the view, so an answer redraws the
 * regions that read what moved and not the whole project window.
 *
 * **And once more each time the window says it changed the workspace** — its `again`
 * (charter-app#174). What moves it is a worktree row the operator ran, which is not a
 * per-render or per-keystroke event.
 *
 * **And `workspace_panels` alone once more each time the plane changes on disk**
 * (charter-app#264). A todo closed in a terminal used to stay on the panel until the
 * workspace was focused again, because nothing else re-ran this. The core debounces what it
 * reports, and the git asks do not follow it.
 *
 * A late answer for a workspace that is no longer focused is dropped three times over — by
 * the ask it answers no longer being the latest, by the name the answer itself carries, and by
 * the state being keyed on the workspace, so nothing left over from the last one is ever drawn
 * under this one's heading.
 */
export type WorkspaceState = {
  /** What the plane says, without git. */
  panels?: PanelsModel;
  /** What git says about each clone, and what the forge cache holds for it. */
  repos?: RepoStates;
  /** This workspace's pieces, by clone name. A clone whose listing has not come back is
   *  absent from the map, which is what `readingPieces` is for. */
  pieces: Record<string, Piece[]>;
  /** Why a clone's pieces could not be listed, by clone name. Shown, never dropped: a repo
   *  with no worktree rows would otherwise read as a repo nobody has cut one in. */
  piecesRefused: Record<string, string>;
  /** Whatever refused the workspace outright, in the core's own words. */
  trouble?: string;
  /** Whether the answer that runs git is still on its way. */
  reading: boolean;
};

/**
 * **Whether any read of the workspace stands refused** (#1244): the workspace itself, the forge
 * cache, a repo the plane will not read, a working tree git could not read, or a clone whose
 * branches could not be listed. Each is said in the Changes view, and each is asked again by
 * the same Read again.
 */
export function readRefusedIn(state: WorkspaceState): boolean {
  return (
    state.trouble !== undefined ||
    (state.repos?.cache_refused ?? null) !== null ||
    (state.panels?.refused.length ?? 0) > 0 ||
    (state.repos?.repos.some((repo) => repo.unreadable !== null) ?? false) ||
    Object.keys(state.piecesRefused).length > 0
  );
}
