// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A piece's files, and one file of a piece, as views** (RC-5): what the light editor's two
 * tabs are about, as `ViewRef`s like every other view (`tabs.ts`), so they open, dedupe and
 * come back at a launch the way every view does.
 *
 * The key is the piece, `workspace/repo/piece`, and for a file the path after it. None of the
 * three names can hold a `/` (`worktree::path_for` refuses one), so the key splits back
 * exactly. The repo's own folder (#948) has no piece, and its key leaves that part empty —
 * `workspace/repo/` — which no piece's name can be. This module draws nothing and imports no
 * editor code: `Views.tsx` reads it to decide which tab to draw before the editor's chunk is
 * loaded.
 *
 * **A file of the plane root is a view of its own kind** (issue 1916), keyed by its path alone:
 * the plane root is no workspace's and no repo's, so no key of the three names spells it, and
 * nothing that renames a workspace touches it.
 */
import type { ViewRef } from "./tabs";

/** Whose files: a piece of a repo, or — with no piece — the repo's own folder. A `Cut` is one. */
export type Place = { workspace: string; repo: string; piece: string | null };

/**
 * **The plane root, as a place whose files the window reads** (issue 1916): the project's own
 * folder, of no workspace and no repo. The window's name for it only: the core is asked for its
 * files by commands of their own (`plane_tree`, `plane_file`), and no command that takes a
 * workspace and a repo reads it.
 */
export const PLANE_ROOT: Place = { workspace: "", repo: "", piece: null };

/** Whether `place` is {@link PLANE_ROOT}. No workspace and no repo has an empty name. */
export function isPlaneRoot(place: Place): boolean {
  return place.workspace === "" && place.repo === "" && place.piece === null;
}

/** What the plane root is called on screen (CONTEXT.md). */
const PLANE_ROOT_NAME = "Plane root";

/** What the place is called on screen: its branch folder's name, or the repo's. */
export function placeName(place: Place): string {
  return isPlaneRoot(place) ? PLANE_ROOT_NAME : (place.piece ?? place.repo);
}

/** The place's part of a key. */
function keyOf(place: Place): string {
  return `${place.workspace}/${place.repo}/${place.piece ?? ""}`;
}

const FILES = "piece-files";
const FILE = "piece-file";
const DIFF = "piece-diff";
/** One file of the plane root, keyed by its path. */
export const PLANE_FILE = "plane-file";

/** The view listing a piece's files, with the light editor beside the list. */
export function pieceFilesView(place: Place): ViewRef {
  return { from: null, view: FILES, key: keyOf(place) };
}

/** One file of a piece, in a tab of its own. */
export function pieceFileView(place: Place, path: string): ViewRef {
  if (isPlaneRoot(place)) return { from: null, view: PLANE_FILE, key: path };
  return { from: null, view: FILE, key: `${keyOf(place)}/${path}` };
}

/** One file of a piece against the branch's base, in a tab of its own: "Show what changed"
 *  (FM-11). */
export function pieceDiffView(place: Place, path: string): ViewRef {
  return { from: null, view: DIFF, key: `${keyOf(place)}/${path}` };
}

/** What a file's comparison tab is called: what it shows, the file's name, and the piece. */
export function pieceDiffTitle(place: Place, path: string): string {
  return `What changed · ${path.slice(path.lastIndexOf("/") + 1)} · ${placeName(place)}`;
}

/** The piece and the file a comparison view is about; `undefined` for every other view. */
export function pieceDiffOf(view: ViewRef): { place: Place; path: string } | undefined {
  if (view.from !== null || view.view !== DIFF) return undefined;
  const of = pieceOf({ ...view, view: FILE });
  return of?.path === undefined ? undefined : { place: of.place, path: of.path };
}

/**
 * **Whether a view is one file in an editor** (#2914): a piece's file, a plane root's file, or a
 * file's comparison. Its tab is a file tab, which joins the editor's toolbar under it.
 */
export function isFileView(view: ViewRef): boolean {
  return (
    view.from === null && (view.view === FILE || view.view === DIFF || view.view === PLANE_FILE)
  );
}

/** What a piece tab is called. */
export function pieceFilesTitle(place: Place): string {
  return `Files · ${placeName(place)}`;
}

/** What a file's own tab is called: its name, and the piece it is in. */
export function pieceFileTitle(place: Place, path: string): string {
  return `${path.slice(path.lastIndexOf("/") + 1)} · ${placeName(place)}`;
}

/** The piece, and the file, a view is about; `undefined` for every other view. */
export function pieceOf(view: ViewRef): { place: Place; path?: string } | undefined {
  if (view.from === null && view.view === PLANE_FILE)
    return view.key === "" ? undefined : { place: PLANE_ROOT, path: view.key };
  if (view.from !== null || (view.view !== FILES && view.view !== FILE)) return undefined;
  const [workspace, repo, piece, ...rest] = view.key.split("/");
  if (!workspace || !repo || piece === undefined) return undefined;
  const place = { workspace, repo, piece: piece === "" ? null : piece };
  if (view.view === FILES) return rest.length === 0 ? { place } : undefined;
  const path = rest.join("/");
  return path === "" ? undefined : { place, path };
}
