// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The list family** (#2911, spec #2897, ADR 0104): every list in the window, on React Aria's
 * collections, wrapped here once. A screen imports these and never `react-aria-components`
 * (`eslint.config.js`), and no screen writes a list's role by hand (`purlis/list-family`).
 *
 * Two kinds of list, by what a row is:
 *
 * - **`ListBox` and `ListOption`: a list to choose from** (React Aria's `ListBox`), the WAI-ARIA
 *   listbox. A row is an option and holds nothing that takes the keyboard of its own: a theme in
 *   the gallery, a search hit, a persona to start a chat with.
 * - **`List` and `ListRow`: a list of rows that act** (React Aria's `GridList`), the WAI-ARIA
 *   grid of one column. A row runs something (`onAction`), may be chosen too, and may hold its
 *   own buttons, which ← and → reach: a panel's todos, a chat's browsers, a vault's secrets.
 *
 * Both draw the same row:
 *
 * - **Drawn by its parts** (ADR 0103). The list is `data-scope="list" data-part="body"`; each row
 *   is the family's root, `data-scope="list" data-part="root"` with its `data-size`, and React
 *   Aria's state attributes (`data-hovered`, `data-selected`, `data-focus-visible`,
 *   `data-disabled`); inside it the slots of `Row.tsx` (`lead`, `label`, `description`, `trail`)
 *   and a section's `header`. `list.css` draws the family default and a theme's parts reach the
 *   same attributes.
 * - **Sizes**: `sm` (24 px, a side view's dense list), `md` (28, the default) and `lg` (32, a
 *   dialog's list beside its details). A list takes one size, so a list of mixed rows cannot be
 *   built.
 * - **Nothing moves when a state changes** (#2800): the pointer's pill, the chosen row's wash and
 *   edge and the keyboard's ring, which is drawn inside the row (ADR 0104, 2026-10-22), change a
 *   colour, a ground or a ring, never a size. The rows stand in from the list's edges, so a
 *   pill never touches the panel the list is joined to.
 * - **The keyboard is React Aria's**: ↑ and ↓ move and stop at the ends, Home and End, typing
 *   finds a row by its `textValue`, Enter and Space choose or run; Escape clears a choice of
 *   many. A list keeps one Tab stop, the row last focused.
 * - **Drag with a way that is not a drag**: `dragAndDropHooks` (`useRowDrag`) lets a row be
 *   dragged out, by the pointer and by the keyboard; whatever a drag does, a row's menu or a
 *   button does too.
 */
import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
  type Ref,
} from "react";
import {
  Collection,
  GridList as AriaGridList,
  GridListHeader as AriaGridListHeader,
  GridListItem as AriaGridListItem,
  GridListSection as AriaGridListSection,
  Header as AriaHeader,
  ListBox as AriaListBox,
  ListBoxItem as AriaListBoxItem,
  ListBoxSection as AriaListBoxSection,
  type GridListItemProps as AriaGridListItemProps,
  type GridListProps as AriaGridListProps,
  type Key,
  type ListBoxItemProps as AriaListBoxItemProps,
  type ListBoxProps as AriaListBoxProps,
  type Selection,
} from "react-aria-components";
import { mergeProps, mergeRefs, useLocale } from "react-aria";
import {
  askForAMenu,
  asksForAMenu,
  useContextMenu,
  type ContextMenuOptions,
} from "../menu/ContextMenu";
import { InOption, RowScope, type RowSize } from "./Row";
import { useRowTip, type RowTipped } from "./RowTip";
import type { Tip } from "../tooltip/Tooltip";

export type { Key, Selection };
export { Collection };

/** The size every row of a list takes, which the list hands them. */
export const ListSize = createContext<RowSize>("md");

/**
 * **The rows of a list that fold what is under them** (#3670, `Fold.tsx`), by key: each one's
 * toggle, which → and ← on it run, or none for a row that runs nothing of the list's (a run's
 * closing line). The list's own `onAction` never runs for one of them.
 */
export const ListFolds = createContext<Map<string, ((open: boolean) => void) | undefined>>(
  new Map(),
);

/**
 * **An attribute React Aria does not hand a row, written on its cell** (`aria-expanded` on a row
 * that folds, #3670): a row of a grid takes no `aria-expanded` (a treegrid's would), and its one
 * cell may, so a reader hears the fold's state on the cell the row holds.
 */
export function useSaidOnRow(name: string, value: string) {
  return useCallback(
    (element: HTMLElement | null) => {
      element?.querySelector(':scope > [role="gridcell"]')?.setAttribute(name, value);
    },
    [name, value],
  );
}

/** How long React Aria's typing to find a row lasts after a letter, in ms (`useTypeSelect`). */
const TYPING_WINDOW = 1000;

/**
 * **Space, → and ← on a row that folds** (#3670): Space folds or unfolds it, → unfolds it and ←
 * folds it, where React Aria's grid would walk into the row's controls, of which it has none. Heard before React Aria's own row
 * hears them (a capture handler on an element around the list runs first). Any other key, and a
 * key on anything inside a row, is React Aria's.
 */
function useFoldKeys(folds: Map<string, ((open: boolean) => void) | undefined>) {
  const { direction } = useLocale();
  // When a letter was last typed: a Space within React Aria's typing window goes on finding a row
  // by its words, and folds nothing.
  const typed = useRef(Number.NEGATIVE_INFINITY);
  return (event: ReactKeyboardEvent<HTMLElement>) => {
    if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
    if (event.key.length === 1 && event.key !== " ") {
      typed.current = event.timeStamp;
      return;
    }
    if (event.key === " " && event.timeStamp - typed.current < TYPING_WINDOW) return;
    const row = event.target;
    if (!(row instanceof HTMLElement) || row.getAttribute("role") !== "row") return;
    const toggle = folds.get(row.getAttribute("data-fold") ?? "");
    if (toggle === undefined) return;
    const open = row.hasAttribute("data-expanded");
    const into = direction === "rtl" ? "ArrowLeft" : "ArrowRight";
    const out = direction === "rtl" ? "ArrowRight" : "ArrowLeft";
    // Space as well as Enter: React Aria's row runs its action on Enter alone.
    if (event.key === " " || (event.key === into && !open) || (event.key === out && open)) {
      event.preventDefault();
      event.stopPropagation();
      toggle(!open);
    }
  };
}

/** What both kinds of list take beside React Aria's own props. */
interface Drawn {
  /** Every row's size. */
  size?: RowSize;
  /** A place for the list in its view: margin, height, flex, grid area. Never a look. */
  className?: string;
  style?: CSSProperties;
  /** What the list says where it has no rows: the empty state, or a line. */
  empty?: ReactNode;
  /** The keys the list answers to beside the arrows (`Shift+Enter`), said to a reader. */
  "aria-keyshortcuts"?: string;
  ref?: Ref<HTMLDivElement>;
  /** `data-*` attributes reach the element as they are. */
  [data: `data-${string}`]: string | number | boolean | undefined;
}

/** Shift+F10 or the menu key on a row asks the row for its menu (`menu` on a row). */
function menuFromTheKeys(event: KeyboardEvent) {
  if (event.defaultPrevented || !asksForAMenu(event)) return;
  const target = event.target;
  if (!(target instanceof Element)) return;
  event.preventDefault();
  askForAMenu(target);
}

/**
 * **A collection's element** (#2911), for a list, a tree and a table alike: it says the keys it
 * answers to (React Aria hands a collection only the attributes it knows, so `aria-keyshortcuts`
 * is written on the element itself), and it hears Shift+F10 and the menu key for the row that
 * has the keyboard, which opens that row's menu.
 */
export function useCollectionElement<E extends HTMLElement>(
  keys: string | undefined,
  given: Ref<E> | undefined,
) {
  const says = useCallback(
    (element: E | null) => {
      if (element === null) return;
      if (keys === undefined) element.removeAttribute("aria-keyshortcuts");
      else element.setAttribute("aria-keyshortcuts", keys);
      element.removeEventListener("keydown", menuFromTheKeys);
      element.addEventListener("keydown", menuFromTheKeys);
    },
    [keys],
  );
  return useMemo(() => mergeRefs<E>(says, given ?? null), [says, given]);
}

/** No menu: what a row with none hands the hook it calls anyway. */
const NO_MENU = () => null;

/** **A row's context menu**: the menu, drawn while it is open, or that and where the keyboard
 *  goes once it has closed. */
export type RowMenu = (() => ReactNode) | ContextMenuOptions;

/**
 * **A row's own menu** (#2911): its context menu, opened by a right-click on it, or by Shift+F10
 * or the menu key while it has the keyboard. The handlers go on the row and the menu is drawn
 * inside it (it stands over the window, wherever it is drawn).
 */
export function useRowMenu(menu: RowMenu | undefined) {
  const own = useContextMenu(
    menu === undefined ? { menu: NO_MENU } : typeof menu === "function" ? { menu } : menu,
  );
  return menu === undefined ? { handlers: {}, menu: null } : own;
}

/**
 * **What a row's element holds**: its words, its menu while it is open, and its tooltip's
 * anchor and tooltip, which shows while the keyboard is on the row.
 */
export function rowHolds<S extends { isFocusVisible: boolean }>(
  children: ReactNode | ((state: S) => ReactNode),
  menu: ReactNode,
  tip: RowTipped,
): (state: S) => ReactNode {
  return (state: S) => (
    <>
      {typeof children === "function" ? children(state) : children}
      {menu}
      {tip.drawn(state.isFocusVisible)}
    </>
  );
}

// --- A list to choose from --------------------------------------------------------------------

export interface ListBoxProps<T extends object>
  extends Drawn, Omit<AriaListBoxProps<T>, "className" | "style" | "renderEmptyState"> {}

/**
 * **A list to choose from**: one option, or many (`selectionMode`), or none that stays chosen
 * (`onAction`, a list that opens what is picked). `layout="grid"` lays its options out in a grid
 * of cards, which the arrows walk in two directions.
 *
 * **A choice of one follows the keyboard** (the WAI-ARIA listbox's single select): the arrows
 * choose as they move, and Enter runs `onAction` on the chosen one.
 */
export function ListBox<T extends object>({
  size = "md",
  className,
  empty,
  selectionBehavior,
  "aria-keyshortcuts": keys,
  ref,
  ...rest
}: ListBoxProps<T>) {
  const own = useCollectionElement(keys, ref);
  return (
    <RowScope.Provider value="list">
      <ListSize.Provider value={size}>
        <AriaListBox
          {...rest}
          ref={own}
          selectionBehavior={
            selectionBehavior ?? (rest.selectionMode === "single" ? "replace" : undefined)
          }
          className={className}
          data-scope="list"
          data-part="body"
          data-size={size}
          renderEmptyState={empty === undefined ? undefined : () => empty}
        />
      </ListSize.Provider>
    </RowScope.Provider>
  );
}

export interface ListOptionProps<T extends object> extends Omit<
  AriaListBoxItemProps<T>,
  "className" | "style"
> {
  /** Its context menu: a right-click, Shift+F10 or the menu key opens it. */
  menu?: RowMenu;
  /** What it says that its words do not, beside it: under the pointer, and while the keyboard
   *  is on it (the shared tooltip, `RowTip.tsx`). */
  tip?: Tip;
  /**
   * **It cannot be chosen now, and says so** (`aria-disabled`, the faint tier): it keeps the
   * keyboard, so its description, which says why, is read. Its owner runs nothing for it.
   */
  unavailable?: boolean;
  /** A place for the row in its list: never a look. */
  className?: string;
  style?: CSSProperties;
  ref?: Ref<HTMLDivElement>;
  /** `data-*` attributes reach the element as they are. */
  [data: `data-${string}`]: string | number | boolean | undefined;
}

/** React Aria keeps an option that is disabled for its choice alone from saying so: said here. */
function useSaysUnavailable(unavailable: boolean, given: Ref<HTMLDivElement> | undefined) {
  const says = useCallback(
    (element: HTMLDivElement | null) => {
      if (element === null) return;
      if (unavailable) element.setAttribute("aria-disabled", "true");
      else element.removeAttribute("aria-disabled");
    },
    [unavailable],
  );
  return useMemo(() => mergeRefs<HTMLDivElement>(says, given ?? null), [says, given]);
}

/**
 * **One option**: its `RowLabel` is its name and its `RowDescription` its description, to a
 * reader too; its `textValue` is what typing finds it by.
 */
export function ListOption<T extends object>({
  className,
  menu,
  tip,
  unavailable = false,
  ref,
  children,
  ...rest
}: ListOptionProps<T>) {
  const size = useContext(ListSize);
  const own = useRowMenu(menu);
  const tipped = useRowTip(tip);
  const element = useSaysUnavailable(unavailable, ref);
  return (
    <AriaListBoxItem
      {...mergeProps(rest, own.handlers, tipped.hover)}
      ref={element}
      className={className}
      data-scope="list"
      data-part="root"
      data-variant="row"
      data-size={size}
      data-unavailable={unavailable || undefined}
    >
      {(state) => (
        <InOption.Provider value={true}>
          {rowHolds(children, own.menu, tipped)(state)}
        </InOption.Provider>
      )}
    </AriaListBoxItem>
  );
}

/** **A group of options under its name**: the name is the section's `header` part. */
export function ListSection<T extends object>({
  title,
  named = false,
  className,
  children,
  "aria-label": label,
  id,
}: {
  /** The group's name, drawn over its options; a group with no name says one to a reader. */
  title?: ReactNode;
  /** The name is data (a file's, a day's), drawn as it is written; else purlis's own label. */
  named?: boolean;
  className?: string;
  children: ReactNode;
  "aria-label"?: string;
  id?: Key;
}) {
  return (
    <AriaListBoxSection<T>
      id={id}
      className={className}
      aria-label={label}
      data-scope="list"
      data-part="section"
    >
      {title !== undefined && (
        <AriaHeader data-scope="list" data-part="header" data-named={named || undefined}>
          {title}
        </AriaHeader>
      )}
      {children}
    </AriaListBoxSection>
  );
}

// --- A list of rows that act -----------------------------------------------------------------

export interface ListProps<T extends object>
  extends Drawn, Omit<AriaGridListProps<T>, "className" | "style" | "renderEmptyState"> {}

/**
 * **A list of rows that act**: each row runs something (`onAction` on the list or on the row),
 * may be chosen (`selectionMode`), and may hold its own buttons, which ← and → reach.
 */
export function List<T extends object>({
  size = "md",
  className,
  empty,
  "aria-keyshortcuts": keys,
  ref,
  onAction,
  ...rest
}: ListProps<T>) {
  const own = useCollectionElement(keys, ref);
  const [folds] = useState(() => new Map<string, ((open: boolean) => void) | undefined>());
  const onKeyDownCapture = useFoldKeys(folds);
  return (
    // An element around the list that draws nothing, so → and ← on a row that folds are heard
    // before React Aria's row hears them (`useFoldKeys`).
    <div data-scope="list" data-part="keys" onKeyDownCapture={onKeyDownCapture}>
      <ListFolds.Provider value={folds}>
        <RowScope.Provider value="list">
          <ListSize.Provider value={size}>
            <AriaGridList
              {...rest}
              // A row that folds runs only its fold (`Fold.tsx`).
              onAction={
                onAction === undefined
                  ? undefined
                  : (key) => {
                      if (!folds.has(String(key))) onAction(key);
                    }
              }
              ref={own}
              className={className}
              data-scope="list"
              data-part="body"
              data-size={size}
              renderEmptyState={empty === undefined ? undefined : () => empty}
            />
          </ListSize.Provider>
        </RowScope.Provider>
      </ListFolds.Provider>
    </div>
  );
}

/** **How a list's row is drawn** (#3669): a row of one line, or a card of a side view's list
 *  (`ListCardRow`), whose height follows its lines. */
export const LIST_ROW_VARIANTS = ["row", "card"] as const;
export type ListRowVariant = (typeof LIST_ROW_VARIANTS)[number];

export interface ListRowProps<T extends object> extends Omit<
  AriaGridListItemProps<T>,
  "className" | "style"
> {
  /** A row of one line (the default), or a card (`ListCardRow`, #3669). */
  variant?: ListRowVariant;
  /** Its context menu: a right-click, Shift+F10 or the menu key opens it. */
  menu?: RowMenu;
  /** What it says that its words do not, beside it: under the pointer, and while the keyboard
   *  is on it (the shared tooltip, `RowTip.tsx`). */
  tip?: Tip;
  /** A place for the row in its list: never a look. */
  className?: string;
  style?: CSSProperties;
  ref?: Ref<HTMLDivElement>;
  /** `data-*` attributes reach the element as they are. */
  [data: `data-${string}`]: string | number | boolean | undefined;
}

/** **One row**: its `textValue` is what typing finds it by and what a reader hears first. */
export function ListRow<T extends object>({
  className,
  variant = "row",
  menu,
  tip,
  children,
  ...rest
}: ListRowProps<T>) {
  const size = useContext(ListSize);
  const own = useRowMenu(menu);
  const tipped = useRowTip(tip);
  return (
    <AriaGridListItem
      {...mergeProps(rest, own.handlers, tipped.hover)}
      className={className}
      data-scope="list"
      data-part="root"
      data-variant={variant}
      data-size={size}
    >
      {rowHolds(children, own.menu, tipped)}
    </AriaGridListItem>
  );
}

/** **A group of rows under its name** (a day's todos): the name is the section's `header`. */
export function ListRowSection<T extends object>({
  title,
  named = false,
  count,
  className,
  children,
  "aria-label": label,
  id,
}: {
  title?: ReactNode;
  /** The name is data (a file's, a day's), drawn as it is written; else purlis's own label. */
  named?: boolean;
  /** How many rows the group holds, at the end of its name (#3340): seen, and not part of the
   *  name a screen reader is told, which is the group's name alone. */
  count?: number;
  className?: string;
  children: ReactNode;
  "aria-label"?: string;
  id?: Key;
}) {
  return (
    <AriaGridListSection<T>
      id={id}
      className={className}
      aria-label={label}
      data-scope="list"
      data-part="section"
    >
      {title !== undefined && (
        <AriaGridListHeader
          data-scope="list"
          data-part="header"
          data-named={named || undefined}
          data-counted={count === undefined ? undefined : ""}
        >
          {count === undefined ? (
            title
          ) : (
            <>
              <span>{title}</span>
              <span data-scope="list" data-part="count" data-count="" aria-hidden="true">
                {count}
              </span>
            </>
          )}
        </AriaGridListHeader>
      )}
      {children}
    </AriaGridListSection>
  );
}
