// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef } from "react";
import { type DragDropEvent } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { untilStopped } from "./here";

/**
 * **Files dropped from the file manager onto a workspace's own files** (#5378): where a drop
 * landed, said by the rows it can land on, and the row drawn as the drop's target while the drag
 * is over it.
 *
 * The system tells the drag through Tauri's own drag events, at a point Tauri calls physical
 * but wry measures as each platform does ({@link pagePoint}); the row under it is the one the
 * page draws there. A drag that carries no files, as one inside the window does, is no drop. A row says what a drop on it does with two
 * attributes ({@link dropOn}): `data-drop-into`, the folder it imports into, or
 * `data-drop-refused`, the sentence that says why it takes none. A point in the view on no row
 * is the workspace's top. A point outside the view is no drop of this view's: it is left alone.
 *
 * The files themselves never cross here: the core holds what the system dropped, and the window
 * says only where (`workspace_files_import_dropped`).
 */

/** Where a drop landed: a folder of the person's own (`""` for the top), or a row that takes
 *  none, with why. */
export type FinderDrop = { into: string } | { refused: string };

/** What a row says of a drop on it, as its attributes: the folder it imports into, or why it
 *  takes none. */
export function dropOn(drop: FinderDrop | undefined): {
  "data-drop-into"?: string;
  "data-drop-refused"?: string;
} {
  if (drop === undefined) return {};
  return "into" in drop ? { "data-drop-into": drop.into } : { "data-drop-refused": drop.refused };
}

/** The attribute that draws a row, or the tree, as a drop's target (the tree family's own). */
const TARGET = "data-drop-target";

/**
 * The page's point for the point the system tells a drag at (wry 0.57). On macOS it is the
 * view's own points (`draggingLocation`), and on Linux GTK's logical pixels: both are the
 * page's. On Windows it is the client area's physical pixels (`ScreenToClient`), which the
 * page's scale divides. Tauri calls all three physical, unscaled.
 */
function pagePoint(point: { x: number; y: number }): { x: number; y: number } {
  if (!navigator.platform.startsWith("Win")) return point;
  const scale = window.devicePixelRatio || 1;
  return { x: point.x / scale, y: point.y / scale };
}

/** Where the drop at the system's `point` lands in `view`, and what to draw as its target:
 *  none outside the view. */
export function landing(
  view: HTMLElement,
  point: { x: number; y: number },
): { drop: FinderDrop; target: HTMLElement | null } | undefined {
  const at = pagePoint(point);
  const under = document.elementFromPoint(at.x, at.y);
  if (under === null || !view.contains(under)) return undefined;
  const row = under.closest<HTMLElement>("[data-drop-into], [data-drop-refused]");
  const top = view.querySelector<HTMLElement>('[role="treegrid"]');
  if (row === null || !view.contains(row)) return { drop: { into: "" }, target: top };
  const refused = row.getAttribute("data-drop-refused");
  if (refused !== null) return { drop: { refused }, target: null };
  const into = row.getAttribute("data-drop-into") ?? "";
  return { drop: { into }, target: folderRow(view, row, into) ?? top };
}

/** The row drawn as the target of a drop into `into`: the row itself where it is that folder's,
 *  else that folder's own row (a file's drop lands in the folder that holds it), else none (the
 *  top's). */
function folderRow(view: HTMLElement, row: HTMLElement, into: string): HTMLElement | null {
  if (into === "") return null;
  if (row.hasAttribute("data-drop-folder")) return row;
  return (
    [...view.querySelectorAll<HTMLElement>("[data-drop-folder]")].find(
      (one) => one.getAttribute("data-drop-into") === into,
    ) ?? null
  );
}

/**
 * **Hear the file manager's drops onto the element `view` finds** while `onDrop` is given: the target drawn while a
 * drag is over the view, put away when it leaves or drops, and each drop in the view handed to
 * `onDrop`. Where the window has no webview to hear (a story, a test without one), nothing is
 * heard.
 */
export function useFinderDrop(
  view: () => HTMLElement | null,
  onDrop: ((drop: FinderDrop) => void) | undefined,
) {
  const heard = useRef(onDrop);
  const viewed = useRef(view);
  useEffect(() => {
    heard.current = onDrop;
    viewed.current = view;
  }, [onDrop, view]);
  const on = onDrop !== undefined;

  useEffect(() => {
    if (!on) return;
    let drawn: HTMLElement | null = null;
    // Whether the drag now over the window carries files: its `over`s say none of their own.
    let files = false;
    const draw = (target: HTMLElement | null) => {
      if (drawn === target) return;
      drawn?.removeAttribute(TARGET);
      target?.setAttribute(TARGET, "");
      drawn = target;
    };
    const hear = ({ payload }: { payload: DragDropEvent }) => {
      const here = viewed.current();
      if (payload.type === "enter") files = payload.paths.length > 0;
      if (payload.type === "drop") files = payload.paths.length > 0;
      if (payload.type === "leave" || here === null || !files) {
        draw(null);
        return;
      }
      const landed = landing(here, payload.position);
      if (payload.type !== "drop") {
        draw(landed?.target ?? null);
        return;
      }
      draw(null);
      if (landed !== undefined) heard.current?.(landed.drop);
    };
    let stop: (() => void) | undefined;
    try {
      // The window's, not its webview's: a window that holds an artifact's child webview is
      // told a drop as a window (#5952), and one that holds none is told it by its label, which
      // a window's listener hears too.
      stop = untilStopped(getCurrentWindow().onDragDropEvent(hear));
    } catch {
      // No webview to hear: nothing is dropped here.
    }
    return () => {
      stop?.();
      draw(null);
    };
  }, [on]);
}

/** Where a row of a file tree is: its workspace (`""` at the plane root), repo and branch. */
type RowPlace = { workspace: string; repo: string; piece: string | null };

/**
 * **What a drop on a row does** (#5378), by the rules Import files… keeps (ADR 0081, amended
 * 2026-11-05): a folder of the person's own takes the files, a file of theirs hands them to its
 * folder, and a repo, a branch's folder, `repos/` and what purlis keeps take none. Nothing at the
 * plane root, whose files take no drop.
 */
export function rowDrop(
  place: RowPlace,
  path: string,
  row: { folder: boolean; name: string; managed?: boolean; reposFolder?: boolean },
): FinderDrop | undefined {
  if (place.workspace === "") return undefined;
  if (place.piece !== null)
    return {
      refused:
        "purlis imports nothing into a branch's folder, which its chats work in. Drop the files on a folder of your own.",
    };
  if (place.repo !== "")
    return {
      refused: `purlis imports nothing into the repo ${place.repo}. Drop the files on a folder of your own, or copy them into the repo in your file manager.`,
    };
  if (row.reposFolder === true)
    return {
      refused:
        "purlis imports nothing into repos/, which holds the workspace's repos. Drop the files on a folder of your own.",
    };
  if (row.managed === true)
    return {
      refused: `purlis keeps ${row.name} itself, and imports nothing into it. Drop the files on a folder of your own.`,
    };
  return { into: row.folder ? path : path.slice(0, Math.max(0, path.lastIndexOf("/"))) };
}
