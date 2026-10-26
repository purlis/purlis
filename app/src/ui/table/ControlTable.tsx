// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A table of controls** (#2911 part 3): a settings table whose cells hold fields, selects and
 * numbers (the dispatch limits, the dispatch grants), or a table whose rows hold pickers and are
 * dragged (the Skill studio's skills). A plain HTML table, so each control keeps its own keys: a
 * field's ← and → move its caret and a select's ↑ ↓ open it, which a grid would take for its own
 * (`Table` is the grid, for a table of data). Each control is its own Tab stop and names its
 * column and its row in full (`aria-label`), so a reader hears the same in either form.
 *
 * Drawn by the family's parts as the data table is (`frame`, `head`, the row, `cell`), in its
 * frame. **It stacks into a form where its room is narrow** (`stacks`, #1859): under 32rem of
 * frame the columns' names are put away, each row's heading takes a line, and each cell sits
 * under its column's whole words (the cell's `heading`), as many side by side as fit; the row's
 * actions take a line of their own. Nothing is cut and nothing scrolls sideways.
 */
import type { CSSProperties, DragEventHandler, ReactNode } from "react";
import { Tooltip } from "../tooltip/Tooltip";
import { Rule, type CellAlign } from "./Table";

/** What a part of the table takes beside what it holds: a place, never a look. */
interface Placed {
  children?: ReactNode;
  /** A place for it in its view (a width, a house meaning such as "dormant"). Never a look. */
  className?: string;
  style?: CSSProperties;
  /** `data-*` attributes reach the element as they are. */
  [data: `data-${string}`]: string | number | boolean | undefined;
}

/**
 * **A table of controls**, in its frame. `fixed` for columns the view sizes itself, which keep
 * their widths whatever the cells hold; `stacks` for one that becomes a form where it is narrow.
 */
export function ControlTable({
  children,
  caption,
  fixed = false,
  stacks = false,
  className,
  style,
  "aria-label": label,
  "aria-describedby": describedBy,
  ...data
}: Placed & {
  /** What the table lists, said above it in the quiet tier. */
  caption?: ReactNode;
  fixed?: boolean;
  stacks?: boolean;
  "aria-label"?: string;
  "aria-describedby"?: string;
}) {
  return (
    <div
      data-scope="table"
      data-part="frame"
      data-stacks={stacks || undefined}
      className={className}
      style={style}
    >
      <table
        {...data}
        data-scope="table"
        data-part="body"
        data-controls=""
        data-fixed={fixed || undefined}
        data-stacks={stacks || undefined}
        aria-label={label}
        aria-describedby={describedBy}
      >
        {caption !== undefined && <caption data-part="caption">{caption}</caption>}
        {children}
      </table>
    </div>
  );
}

/** **The columns' names**: `ControlColumn`s, in one row over the table. */
export function ControlHead({ children }: { children: ReactNode }) {
  return (
    <thead data-scope="table" data-part="header">
      <tr>{children}</tr>
    </thead>
  );
}

/**
 * **One column's name**, on one line. A name shortened to fit its column (`whole` its whole
 * words) or with more to say (`about`) shows its words behind the shared tooltip (#2908), which
 * makes them a Tab stop, and its whole words name the column for a reader (DS-9, #1841).
 */
export function ControlColumn({
  children,
  whole,
  about,
  align = "start",
  actions = false,
  hideName = false,
  className,
}: {
  /** The words it shows. */
  children?: string;
  /** Its whole words, where `children` shortens them. */
  whole?: string;
  /** What the column holds, in a sentence. */
  about?: string;
  align?: CellAlign;
  /** The column of a row's buttons: as narrow as they are, at the row's end. */
  actions?: boolean;
  /** A column named for a reader only: its name is not drawn. */
  hideName?: boolean;
  className?: string;
}) {
  const words = children ?? "";
  const full = whole ?? words;
  const shortened = full !== words;
  const tipped = shortened || about !== undefined;
  return (
    <th
      scope="col"
      className={className}
      data-scope="table"
      data-part="head"
      data-align={align}
      data-actions={actions || undefined}
      aria-label={hideName ? full || "Actions" : shortened ? full : undefined}
    >
      <Rule />
      {hideName ? null : tipped ? (
        // Words, not a control: they press nothing, so they are no button.
        <Tooltip
          tip={
            about === undefined ? (
              full
            ) : (
              <>
                <strong>{full}</strong> {about}
              </>
            )
          }
        >
          <span data-part="head-tip">{words}</span>
        </Tooltip>
      ) : (
        words
      )}
    </th>
  );
}

/** **A group of rows**: the table's rows, or one group of them under its `ControlGroupHeading`. */
export function ControlBody({ children, className, style, ...data }: Placed) {
  return (
    <tbody {...data} className={className} style={style} data-scope="table" data-part="rows">
      {children}
    </tbody>
  );
}

/** **One row**: its heading, then a cell per column. A row the view lets be dragged says so. */
export function ControlRow({
  children,
  className,
  style,
  draggable,
  onDragStart,
  ...data
}: Placed & { draggable?: boolean; onDragStart?: DragEventHandler<HTMLTableRowElement> }) {
  return (
    <tr
      {...data}
      className={className}
      style={style}
      draggable={draggable}
      onDragStart={onDragStart}
      data-scope="table"
      data-part="root"
    >
      {children}
    </tr>
  );
}

/** **The cell that names its row** (a reader hears it with every cell of the row). */
export function ControlRowHeading({
  children,
  rowSpan,
  className,
  ...data
}: Placed & { rowSpan?: number }) {
  return (
    <th
      {...data}
      scope="row"
      rowSpan={rowSpan}
      className={className}
      data-scope="table"
      data-part="cell"
      data-names-row=""
    >
      {children}
      <Rule />
    </th>
  );
}

/** **A group's name**, across the table, over the rows under it. */
export function ControlGroupHeading({
  children,
  colSpan,
  className,
}: Placed & { colSpan: number }) {
  return (
    <th
      scope="rowgroup"
      colSpan={colSpan}
      className={className}
      data-scope="table"
      data-part="cell"
      data-group=""
    >
      {children}
      <Rule />
    </th>
  );
}

/** Whether a cell holds nothing (the stacked form draws no such cell). */
function empty(children: ReactNode): boolean {
  if (Array.isArray(children)) return children.every(empty);
  return children === null || children === undefined || children === false || children === "";
}

/**
 * **One cell**: a control, or words. `heading` is its column's whole words, which the stacked
 * form shows over it; `actions` the row's buttons at its end; `colSpan` a cell across the
 * columns (a question about the row above it).
 */
export function ControlCell({
  children,
  heading,
  actions = false,
  align = "start",
  colSpan,
  className,
  style,
  ...data
}: Placed & { heading?: string; actions?: boolean; align?: CellAlign; colSpan?: number }) {
  return (
    <td
      {...data}
      colSpan={colSpan}
      className={className}
      style={style}
      data-scope="table"
      data-part="cell"
      data-heading={heading}
      data-actions={actions || undefined}
      data-align={align}
      data-empty={empty(children) || undefined}
    >
      {children}
      <Rule />
    </td>
  );
}
