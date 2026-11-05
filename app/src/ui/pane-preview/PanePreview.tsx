// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a drag draws over the panes** (#5291, spec #5288; ADR 0104), wrapped here once. A screen
 * imports these and never writes `data-scope="pane-preview"` (`purlis/pane-preview-family`).
 *
 * - **`PanePreviewArriving`**: a pane the drop would make, or one with no frame on screen: a chat
 *   from the list, or a side that would fold (`as` a rail or a bar).
 * - **`PanePreviewSize`**: the columns and rows a terminal will take, on the pane that will change.
 * - **`PanePreviewRefused`**: the mark on the pane a refused place means.
 * - **`PanePreviewHeld`**: what is held, following the pointer.
 * - **`PanePreviewWords`**: what letting go would do, or why it can't, beside the pointer.
 *
 * **None is read out or takes the pointer.** A reader hears the drag from the window's own status
 * line, and the pointer has to reach what is under them. Each sits at a place the drag measured,
 * given as its `box` or set from where the pointer is.
 *
 * **Drawn by its parts** (ADR 0103): `pane-preview.css` draws the family's default, and a theme's
 * `pane-preview` parts reach the same attributes.
 */
import type { CSSProperties, ReactNode, Ref } from "react";

/** Where a part sits, in the window's own pixels. */
export type PreviewBox = { left: number; top: number; width: number; height: number };

const placed = (box: PreviewBox): CSSProperties => ({
  left: box.left,
  top: box.top,
  width: box.width,
  height: box.height,
});

/** **A pane about to be**, at the box it would take, with its name, or as a side folded small. */
export function PanePreviewArriving({
  box,
  name,
  as = "pane",
}: {
  box: PreviewBox;
  name?: string;
  as?: "pane" | "rail" | "bar";
}) {
  return (
    <div
      data-scope="pane-preview"
      data-part="arriving"
      data-variant={as}
      aria-hidden="true"
      style={placed(box)}
    >
      {name !== undefined && (
        <span data-scope="pane-preview" data-part="name">
          {name}
        </span>
      )}
    </div>
  );
}

/** **The size a terminal will take**, in the top corner of the `box` its pane would have. */
export function PanePreviewSize({
  box,
  columns,
  rows,
}: {
  box: PreviewBox;
  columns: number;
  rows: number;
}) {
  return (
    <div
      data-scope="pane-preview"
      data-part="size"
      aria-hidden="true"
      style={{ left: box.left + box.width, top: box.top }}
    >
      {columns} × {rows}
    </div>
  );
}

/** **The pane a refused place means**: its edge, over where it is. */
export function PanePreviewRefused({ box }: { box: PreviewBox }) {
  return (
    <div data-scope="pane-preview" data-part="refused" aria-hidden="true" style={placed(box)} />
  );
}

/** **What is held**, its name, by the pointer. */
export function PanePreviewHeld({
  children,
  at,
  ref,
}: {
  children: ReactNode;
  at: { left: number; top: number };
  ref?: Ref<HTMLDivElement>;
}) {
  return (
    <div ref={ref} data-scope="pane-preview" data-part="held" aria-hidden="true" style={at}>
      {children}
    </div>
  );
}

/** **What letting go would do**, or, `refused`, why it can't, by the pointer. */
export function PanePreviewWords({
  children,
  refused = false,
  at,
  ref,
}: {
  children: ReactNode;
  refused?: boolean;
  at: { left: number; top: number };
  ref?: Ref<HTMLDivElement>;
}) {
  return (
    <div
      ref={ref}
      data-scope="pane-preview"
      data-part="words"
      data-refused={refused || undefined}
      aria-hidden="true"
      style={at}
    >
      {children}
    </div>
  );
}
