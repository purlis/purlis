// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef } from "react";
import { getCurrentWebview, type DragDropEvent } from "@tauri-apps/api/webview";

/**
 * **Files dropped from the file manager onto a workspace's own files** (#5378): where a drop
 * landed, said by the rows it can land on, and the row drawn as the drop's target while the drag
 * is over it.
 *
 * The system tells the drag through Tauri's own drag events, at a point in physical pixels; the
 * row under it is the one the page draws there. A row says what a drop on it does with two
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

/** Where the drop at `point`, in physical pixels, lands in `view`, and what to draw as its
 *  target: none outside the view. */
export function landing(
  view: HTMLElement,
  point: { x: number; y: number },
): { drop: FinderDrop; target: HTMLElement | null } | undefined {
  const scale = window.devicePixelRatio || 1;
  const under = document.elementFromPoint(point.x / scale, point.y / scale);
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
    const draw = (target: HTMLElement | null) => {
      if (drawn === target) return;
      drawn?.removeAttribute(TARGET);
      target?.setAttribute(TARGET, "");
      drawn = target;
    };
    const hear = ({ payload }: { payload: DragDropEvent }) => {
      const here = viewed.current();
      if (payload.type === "leave" || here === null) {
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
    let gone = false;
    try {
      void getCurrentWebview()
        .onDragDropEvent(hear)
        .then(
          (unlisten) => {
            if (gone) unlisten();
            else stop = unlisten;
          },
          () => {},
        );
    } catch {
      // No webview to hear: nothing is dropped here.
    }
    return () => {
      gone = true;
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
