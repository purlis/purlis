// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The table family** (#2911 part 3, spec #2897, ADR 0104): every table in the window, wrapped
 * here once. A screen imports these and never `react-aria-components` (`eslint.config.js`), and
 * no screen writes a table's element or role by hand (`purlis/table-family`).
 *
 * Two kinds of table, by what a cell holds:
 *
 * - **`Table`: a table of data** (React Aria's `Table`, the WAI-ARIA grid): rows of words, each
 *   cell holding at most a button, a link or a switch. The keyboard is the APG grid's: one Tab
 *   stop; the arrows move a row or a cell at a time and stop at the edges; with a row focused,
 *   Home and End go to the first and the last row, with a cell focused to the row's ends; Ctrl+Home
 *   and Ctrl+End go to the table's ends, Page Up and Page Down a page; a button in a cell takes
 *   the keyboard where the arrows reach it. A table may choose rows (`selectionMode`, a
 *   check at each row's start and one over them for every row, Space to choose), sort by a
 *   column (`allowsSorting`), open a row (`onRowAction`, Enter) and say what it has where it has
 *   no rows (`empty`).
 * - **`ControlTable` (`./ControlTable`): a table of controls**: settings rows whose cells are
 *   fields, selects and numbers. The grid would take ← and → from a field's caret and ↑ ↓ from a
 *   select, so it is a plain table, its controls each their own Tab stop, and it stacks into a
 *   form where its room is narrow.
 *
 * Both are drawn by their parts (ADR 0103): the `frame` (the card that holds the table and
 * scrolls it sideways), the `head` (a column's name, over its cells), the row (the family's
 * root, with React Aria's state attributes), the `cell`, and the `bar` over a table that chooses.
 * `table.css` draws the family default and a theme's parts reach the same attributes.
 *
 * **A row's menu** (`menu` on `TableRow`, with `rowMenus` on the table): the row's actions, under
 * one ⋯ in a column of its own at the row's end, aligned on every row; a right-click on the row,
 * Shift+F10 or the menu key open the same menu. **Nothing moves when a state changes** (#2800):
 * the pointer's ground, a chosen row's wash and the keyboard's ring, drawn inside the row or the
 * cell, change a colour, never a size.
 */
import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  type ReactNode,
  type Ref,
} from "react";
import {
  Cell as AriaCell,
  Checkbox as AriaCheckbox,
  Column as AriaColumn,
  Row as AriaRow,
  Table as AriaTable,
  TableBody as AriaTableBody,
  TableHeader as AriaTableHeader,
  useTableOptions,
  type CellProps as AriaCellProps,
  type ColumnProps as AriaColumnProps,
  type Key,
  type RowProps as AriaRowProps,
  type Selection,
  type SortDescriptor,
  type TableBodyProps as AriaTableBodyProps,
  type TableProps as AriaTableProps,
} from "react-aria-components";
import { mergeProps, mergeRefs, VisuallyHidden } from "react-aria";
import { ArrowDown, ArrowUp, Check, Ellipsis, Minus } from "lucide-react";
import { IconButton } from "../button/Button";
import { MenuTrigger } from "../menu/Menu";
import { askForAMenu } from "../menu/ContextMenu";
import { useCollectionElement, useRowMenu } from "../list/List";
import { RowScope } from "../list/Row";

export type { Key, Selection, SortDescriptor };

/** What the table hands its header and its rows: whether every row ends in its menu's column. */
const Drawn = createContext({ menus: false });

/**
 * **The rule between rows** (`table.rule`, its own part, #2804): drawn at a cell's top edge, and
 * under a column's name. Purlis draws none (the rows' room, the pointer's ground and the chosen
 * wash carry the bands); Purlis Classic sets the hairline in its theme.
 */
export function Rule() {
  return <span data-scope="table" data-part="rule" aria-hidden="true" />;
}

/** Where a column's words and its cells' words stand: at the start, or at the end (a number). */
export type CellAlign = "start" | "end";

export interface TableProps extends Omit<
  AriaTableProps,
  "className" | "style" | "children" | "render"
> {
  children: ReactNode;
  /**
   * **Every row ends in its menu's column** (`menu` on a row): a ⋯ on each row, aligned, under a
   * column a reader hears as "Actions". A row without a menu keeps its empty cell there.
   */
  rowMenus?: boolean;
  /** What the table lists, said over it in the quiet tier (a table of a group of many). */
  caption?: ReactNode;
  /** The keys the table answers to beside the grid's, said to a reader. */
  "aria-keyshortcuts"?: string;
  /** A place for the table's frame in its view: margin, height, flex. Never a look. */
  className?: string;
  style?: React.CSSProperties;
  ref?: Ref<HTMLTableElement>;
  /** `data-*` attributes reach the table as they are. */
  [data: `data-${string}`]: string | number | boolean | undefined;
}

/**
 * **A table of data**: its `TableHeader` of `TableColumn`s, then its `TableBody` of `TableRow`s
 * of `TableCell`s, one per column. It is drawn in its frame, a card that scrolls it sideways
 * where its room is narrower than the table's least width.
 */
export function Table({
  children,
  rowMenus = false,
  caption,
  className,
  style,
  "aria-keyshortcuts": keys,
  ref,
  ...rest
}: TableProps) {
  const own = useCollectionElement<HTMLTableElement>(keys, ref);
  const drawn = useMemo(() => ({ menus: rowMenus }), [rowMenus]);
  return (
    <RowScope.Provider value="table">
      <Drawn.Provider value={drawn}>
        <div data-scope="table" data-part="frame" className={className} style={style}>
          {caption !== undefined && <p data-part="caption">{caption}</p>}
          <AriaTable {...rest} ref={own} data-scope="table" data-part="body">
            {children}
          </AriaTable>
        </div>
      </Drawn.Provider>
    </RowScope.Provider>
  );
}

/** **The check that chooses a row**, or every row from over them (React Aria's `selection`). */
function Choose() {
  return (
    <AriaCheckbox slot="selection" data-scope="table" data-part="select">
      {({ isSelected, isIndeterminate }) => (
        <span data-part="box" aria-hidden="true">
          {isIndeterminate ? <Minus /> : isSelected ? <Check /> : null}
        </span>
      )}
    </AriaCheckbox>
  );
}

/**
 * **The columns' names**, over the rows: the `TableColumn`s, after the column of checks where
 * the table chooses many rows, and before the column of menus where its rows have them.
 */
export function TableHeader({ children }: { children: ReactNode }) {
  const { selectionBehavior, selectionMode } = useTableOptions();
  const { menus } = useContext(Drawn);
  return (
    <AriaTableHeader data-scope="table" data-part="header">
      {selectionBehavior === "toggle" && (
        <AriaColumn
          id="purlis:choose"
          textValue="Choose"
          data-scope="table"
          data-part="head"
          data-choose=""
        >
          {selectionMode === "multiple" ? <Choose /> : <VisuallyHidden>Choose</VisuallyHidden>}
          <Rule />
        </AriaColumn>
      )}
      {children}
      {menus && (
        <AriaColumn
          id="purlis:menu"
          textValue="Actions"
          data-scope="table"
          data-part="head"
          data-actions=""
        >
          <VisuallyHidden>Actions</VisuallyHidden>
          <Rule />
        </AriaColumn>
      )}
    </AriaTableHeader>
  );
}

export interface TableColumnProps extends Omit<
  AriaColumnProps,
  "className" | "style" | "children" | "render" | "width"
> {
  /** Its name: one line, in the quiet tier, in capitals (the label role, R161). */
  children: ReactNode;
  /** A number's column: its name and its cells at the end. */
  align?: CellAlign;
  /** A column named for a reader only (a column of buttons): its name is not drawn. */
  hideName?: boolean;
}

/**
 * **One column's name.** `isRowHeader` on the column that names each row (a reader hears it
 * with every cell); `allowsSorting` makes the name a button that sorts by it (the table's
 * `sortDescriptor` and `onSortChange`), and its arrow says which way.
 */
export function TableColumn({
  children,
  align = "start",
  hideName = false,
  allowsSorting,
  ...rest
}: TableColumnProps) {
  return (
    <AriaColumn
      {...rest}
      allowsSorting={allowsSorting}
      data-scope="table"
      data-part="head"
      data-align={align}
    >
      {({ sortDirection }) => (
        <>
          {hideName ? <VisuallyHidden>{children}</VisuallyHidden> : children}
          <Rule />
          {/* Its room is kept while it sorts by another column, so sorting moves no name. */}
          {allowsSorting === true && (
            <span data-part="sort" aria-hidden="true" data-sorted={sortDirection ?? undefined}>
              {sortDirection === "descending" ? <ArrowDown /> : <ArrowUp />}
            </span>
          )}
        </>
      )}
    </AriaColumn>
  );
}

export interface TableBodyProps<T extends object> extends Omit<
  AriaTableBodyProps<T>,
  "className" | "style" | "renderEmptyState" | "render"
> {
  /** What the table says where it has no rows: the empty state, or a line. */
  empty?: ReactNode;
}

/** **The rows**: `TableRow`s, or `items` and a function that draws one. */
export function TableBody<T extends object>({ empty, ...rest }: TableBodyProps<T>) {
  return (
    <AriaTableBody
      {...rest}
      data-scope="table"
      data-part="rows"
      renderEmptyState={
        empty === undefined
          ? undefined
          : () => (
              <div data-scope="table" data-part="empty">
                {empty}
              </div>
            )
      }
    />
  );
}

/** React Aria hands a row only the attributes it knows: `aria-current` is said here. */
function useSaysCurrent(current: boolean, given: Ref<HTMLTableRowElement> | undefined) {
  const says = useCallback(
    (element: HTMLTableRowElement | null) => {
      if (element === null) return;
      if (current) element.setAttribute("aria-current", "true");
      else element.removeAttribute("aria-current");
    },
    [current],
  );
  return useMemo(() => mergeRefs<HTMLTableRowElement>(says, given ?? null), [says, given]);
}

export interface TableRowProps<T extends object> extends Omit<
  AriaRowProps<T>,
  "className" | "style" | "children" | "render"
> {
  /** Its cells: one `TableCell` per column, in the columns' order. */
  children: ReactNode;
  /**
   * **Its menu**: the `Menu` of the row's actions, under the ⋯ at its end (the table's
   * `rowMenus`), and opened by a right-click on it, Shift+F10 or the menu key. A row with a menu
   * and no action of its own opens its menu when it is run: Enter on it, or a press.
   */
  menu?: () => ReactNode;
  /** The ⋯'s name: "More for" the row's `textValue`, unless said. */
  menuLabel?: string;
  /** **The row the view is about** (`aria-current`): drawn as a chosen row is. */
  current?: boolean;
  /** A place for the row: never a look. */
  className?: string;
  ref?: Ref<HTMLTableRowElement>;
  /** `data-*` attributes reach the element as they are. */
  [data: `data-${string}`]: string | number | boolean | undefined;
}

/** **One row**: its `textValue` is what typing finds it by, and what its ⋯ is named after. */
export function TableRow<T extends object>({
  children,
  menu,
  menuLabel,
  current = false,
  className,
  ref,
  onAction,
  ...rest
}: TableRowProps<T>) {
  const { selectionBehavior } = useTableOptions();
  const { menus } = useContext(Drawn);
  // A menu is the row's only where the table draws its ⋯ column (`rowMenus`): elsewhere a
  // right-click and the menu key would open nothing, so the row takes neither (D-2911-B-18).
  const menued = menus && menu !== undefined;
  if (import.meta.env.DEV && menu !== undefined && !menus)
    console.warn(
      `TableRow ${rest.textValue ?? ""}: a row's menu needs \`rowMenus\` on its table, which draws its ⋯.`,
    );
  const own = useRowMenu(menued ? menu : undefined);
  const mine = useRef<HTMLTableRowElement | null>(null);
  const element = useSaysCurrent(current, ref);
  const both = useMemo(() => mergeRefs<HTMLTableRowElement>(mine, element), [element]);
  // A row with a menu and no action of its own opens its menu when it is run, on its first row,
  // as its menu from the keys does (D-2911-B-4).
  const runs =
    onAction ??
    (menued
      ? () => {
          if (mine.current !== null) askForAMenu(mine.current);
        }
      : undefined);
  return (
    <AriaRow
      {...mergeProps(rest, own.handlers)}
      onAction={runs}
      ref={both}
      className={className}
      data-scope="table"
      data-part="root"
      data-current={current || undefined}
    >
      {selectionBehavior === "toggle" && (
        <AriaCell data-scope="table" data-part="cell" data-choose="">
          <Choose />
          <Rule />
        </AriaCell>
      )}
      {children}
      {menus && (
        <AriaCell data-scope="table" data-part="cell" data-actions="">
          <Rule />
          {menu !== undefined && (
            <>
              <MenuTrigger>
                <IconButton
                  size="sm"
                  variant="ghost"
                  aria-label={menuLabel ?? `More for ${rest.textValue ?? ""}`.trim()}
                  icon={<Ellipsis aria-hidden="true" />}
                  data-row-menu=""
                />
                {menu()}
              </MenuTrigger>
              {own.menu}
            </>
          )}
        </AriaCell>
      )}
    </AriaRow>
  );
}

export interface TableCellProps extends Omit<
  AriaCellProps,
  "className" | "style" | "children" | "render"
> {
  children?: ReactNode;
  /** A number's cell: at the end, in figures of one width. */
  align?: CellAlign;
  /** A place for what it holds: never a look. */
  className?: string;
  /** `data-*` attributes reach the element as they are. */
  [data: `data-${string}`]: string | number | boolean | undefined;
}

/** **One cell**: its words, or the button, link or switch it holds. */
export function TableCell({ children, align = "start", className, ...rest }: TableCellProps) {
  return (
    <AriaCell
      {...rest}
      className={className}
      data-scope="table"
      data-part="cell"
      data-align={align}
    >
      {children}
      <Rule />
    </AriaCell>
  );
}
