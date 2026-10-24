// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Dragging a row out of a list, a tree or a table** (#2911): React Aria's drag, wrapped once
 * for every collection family. A row dragged out carries what `items` says for it, each by its
 * type, written to the drag as a drop target outside the window's collections reads it (a chat
 * reads a file's reference from its own type).
 *
 * **A drag is never the only way**: whatever dropping a row does, its menu or a button does too
 * (an "Add to a chat" beside the file's row), so the keyboard and a reader do it without one.
 */
import { useDragAndDrop, type DragAndDropHooks, type Key } from "react-aria-components";

/** What a row carries when it is dragged: its data by type, `text/plain` among them. */
export type Carried = Record<string, string>;

/**
 * **The hooks a collection is handed to let its rows be dragged**: `carried` says what each
 * dragged row carries (`undefined` for a row that is not dragged).
 */
export function useRowDrag(carried: (key: Key) => Carried | undefined): DragAndDropHooks {
  return useDragAndDrop({
    getItems: (keys) =>
      [...keys].flatMap((key) => {
        const one = carried(key);
        return one === undefined ? [] : [one];
      }),
    getAllowedDropOperations: () => ["copy"],
  }).dragAndDropHooks;
}
