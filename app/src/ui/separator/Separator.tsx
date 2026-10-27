// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The separator family** (#2912, spec #2897, ADR 0104): the line between two groups inside a
 * surface, on React Aria's `Separator`, wrapped here once. A screen imports this and never draws
 * a line of its own (`purlis/separator-family`).
 *
 * - **Its own part, never a control's border** (#2804, #2877). The line is an element,
 *   `data-scope="separator" data-part="root"` with its `data-orientation`; `separator.css` draws
 *   it in the subtle hairline's colour, the theme's `separator.root.fill`, and as thick as the
 *   theme's `separator.width` geometry, which may be 0: a theme with no lines draws none, and
 *   nothing beside it moves, since its room is its margin's.
 * - **Said or not.** A line between two groups a screen reader reads as groups is a separator
 *   (`role="separator"`, React Aria's); one that only parts two marks on a strip (the activity
 *   bar's halves) is `decorative`, hidden from it.
 * - **A place, never a look**: `className` gives it a margin or an order in its view.
 */
import type { CSSProperties } from "react";
import { Separator as AriaSeparator } from "react-aria-components";

export type SeparatorOrientation = "horizontal" | "vertical";

export function Separator({
  orientation = "horizontal",
  decorative = false,
  className,
  style,
}: {
  /** Across the surface (between rows), or down it (between items of a strip). */
  orientation?: SeparatorOrientation;
  /** A line that parts marks and no groups a screen reader reads: hidden from it. */
  decorative?: boolean;
  /** Its place: a margin, an order. */
  className?: string;
  style?: CSSProperties;
}) {
  if (decorative)
    return (
      <div
        data-scope="separator"
        data-part="root"
        data-orientation={orientation}
        aria-hidden="true"
        className={className}
        style={style}
      />
    );
  return (
    <AriaSeparator
      data-scope="separator"
      data-part="root"
      data-orientation={orientation}
      orientation={orientation}
      // A line down a strip is a `div`: an `hr` is only ever across.
      elementType={orientation === "vertical" ? "div" : "hr"}
      className={className}
      style={style}
    />
  );
}
