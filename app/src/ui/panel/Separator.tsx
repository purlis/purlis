// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ComponentProps } from "react";
import { Separator as ResizableSeparator } from "react-resizable-panels";
import { useProjectShown } from "../../projectShown";

/**
 * **The panel separator family** (#3143, spec #2897): the handle between two panels that a person
 * drags, or moves with the arrow keys, to resize them. Every resize handle of the window is one:
 * a side's edge, the line under the work, the line between the bottom panel's halves, a split
 * between two panes and the file tree's edge.
 *
 * - **Its root is the library's handle**: `data-scope="panel-separator"`, `data-part="root"`.
 *   The library marks its state itself, on `data-separator`: `hover` under the pointer, `active`
 *   while it is dragged, `focus` while it has the keyboard (the window's own ring draws it). A
 *   theme's `panel-separator.*.hover` and `.pressed` keys are written on those
 *   (`purlis_core::theme::parts`).
 * - **Its grip** (`data-part="grip"`) is a short mark at its middle, drawn over what is either
 *   side of it and out of the pointer's way: a wider look never changes the handle's box, so
 *   nothing moves and the place a person can grab stays the same. `panel.css` draws Purlis's:
 *   nothing at rest, the faint tier under the pointer, the muted tier while it is dragged.
 *   Purlis Classic's own parts draw its accent line instead.
 *
 * **Out of the pointer's way while its project is not in front** (`projectShown.tsx`). The
 * library listens on the document and, for every press and move, asks each group it holds
 * whether the pointer is on one of its handles, by where the handle is and by what is drawn over
 * it. A handle in an element that is in no document is at no place at all, which it reads as
 * the window's corner, and has no ancestor in common with what was pressed, which it throws
 * for. A disabled handle is one it does not ask about.
 *
 * **The handle, and not its `Group`.** A handle's `disabled` is a mark the library changes in
 * place, and nothing is laid out for it. A group's registers the group again, which lays the
 * group out again from what it remembers (`RegionFrame.tsx` says what registering again costs
 * in this library).
 */
export function Separator(props: Omit<ComponentProps<typeof ResizableSeparator>, "children">) {
  const shown = useProjectShown();
  return (
    <ResizableSeparator
      {...props}
      data-scope="panel-separator"
      data-part="root"
      disabled={props.disabled === true || !shown}
    >
      <span data-scope="panel-separator" data-part="grip" aria-hidden="true" />
    </ResizableSeparator>
  );
}
