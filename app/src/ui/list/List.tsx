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
 *   many. A list keeps one Tab stop, the row last focused: Tab or Shift+Tab into it lands on
 *   that row, never on a control in it, and → reaches its controls (#6033, `backToTheRow`).
 * - **Drag with a way that is not a drag**: `dragAndDropHooks` (`useRowDrag`) lets a row be
 *   dragged out, by the pointer and by the keyboard; whatever a drag does, a row's menu or a
 *   button does too.
 */
import {
  createContext,
  useCallback,
  useContext,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type JSX,
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
  ListLayout,
  Virtualizer,
  type GridListItemProps as AriaGridListItemProps,
  type GridListItemRenderProps,
  type GridListProps as AriaGridListProps,
  type Key,
  type ListBoxItemProps as AriaListBoxItemProps,
  type ListBoxProps as AriaListBoxProps,
  type Selection,
} from "react-aria-components";
import { mergeProps, mergeRefs, useLocale } from "react-aria";
import {
  askForAMenu,
  MenuOpenedFrom,
  asksForAMenu,
  useContextMenu,
  type ContextMenuOptions,
} from "../menu/ContextMenu";
import {
  heardIds,
  withoutPart,
  withPart,
  InOption,
  RowHeard,
  RowScope,
  type PartHeard,
  type PartsHeard,
  type RowSize,
} from "./Row";
import { useListRowTip, type RowTipped } from "./RowTip";
import {
  isComposing,
  isEscapeInABox,
  keepsItsArrows,
  keptOnTheWayDown,
  movesWithinItsRow,
  movedInARadioGroup,
} from "./rowKeys";
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
 * **Whether the list chooses** (`selectionMode`, #3769): its fold rows then say they are never
 * chosen (`isDisabled`, with the list's `disabledBehavior="selection"`), so a press or a key
 * still folds them and Select all leaves them out.
 */
export const ListChooses = createContext(false);

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
function useFoldKeys(
  folds: Map<string, ((open: boolean) => void) | undefined>,
  findsByTyping: boolean,
) {
  const { direction } = useLocale();
  // When a letter was last typed: a Space within React Aria's typing window goes on finding a row
  // by its words, and folds nothing. A list that finds nothing by typing has no such window.
  const typed = useRef(Number.NEGATIVE_INFINITY);
  return (event: ReactKeyboardEvent<HTMLElement>) => {
    if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
    const row = event.target;
    if (!(row instanceof HTMLElement) || row.getAttribute("role") !== "row") return;
    if (event.key.length === 1 && event.key !== " ") {
      if (findsByTyping) typed.current = event.timeStamp;
      return;
    }
    if (event.key === " " && event.timeStamp - typed.current < TYPING_WINDOW) return;
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
  return menu === undefined ? { handlers: {}, menu: null, openedFrom: null } : own;
}

/** What a row's menu hands what the row holds: the menu, and the button it is open from. */
export interface RowMenuHeld {
  menu: ReactNode;
  openedFrom: HTMLElement | null;
}

/**
 * **What a row's element holds**: its words, its menu while it is open, and its tooltip's
 * anchor and tooltip, which shows while the keyboard is on the row. The buttons in it hear which
 * of them its menu is open from (`MenuOpenedFrom`, #3769).
 */
export function rowHolds<S extends { isFocusVisible: boolean }>(
  children: ReactNode | ((state: S) => ReactNode),
  menu: RowMenuHeld,
  tip: RowTipped,
): (state: S) => ReactNode {
  return (state: S) => (
    <MenuOpenedFrom.Provider value={menu.openedFrom}>
      {typeof children === "function" ? children(state) : children}
      {menu.menu}
      {tip.drawn(state.isFocusVisible)}
    </MenuOpenedFrom.Provider>
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
  "className" | "style" | "render"
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
 * **What a row's lines are heard by** (#6075, #6101): the ids its parts hand it (`PartsHeard`),
 * its tooltip's among them while it is up, in `HEARD_ORDER`'s order, which its element is
 * described by before what React Aria adds.
 */
function useRowHeard(): { heard: PartsHeard; describedBy: string | undefined } {
  const [parts, setParts] = useState<readonly PartHeard[]>([]);
  const base = useId();
  const heard = useMemo<PartsHeard>(
    () => ({
      base,
      add(kind, id) {
        setParts((was) => withPart(was, { kind, id }));
        return () => setParts((was) => withoutPart(was, id));
      },
    }),
    [base],
  );
  const describedBy = useMemo(() => idsOf(...heardIds(parts)), [parts]);
  return { heard, describedBy };
}

/** An option's element, described by its own lines and then by React Aria's (#6075): React
 *  Aria's `useOption` sets `aria-describedby` to its description slot alone, which the family
 *  does not fill, so what it sets is what else React Aria adds (its hint for dragging). */
function optionElement(heard: string | undefined) {
  return (element: Parameters<NonNullable<AriaListBoxItemProps["render"]>>[0]) => {
    const describedBy = idsOf(heard, element["aria-describedby"]);
    // An option with an `href` is a link, which React Aria hands as one.
    return "href" in element ? (
      <a {...element} aria-describedby={describedBy} />
    ) : (
      <div {...element} aria-describedby={describedBy} />
    );
  };
}

/** The ids given, in order, as one `aria-describedby`; none where none is given. */
function idsOf(...given: (string | undefined)[]): string | undefined {
  const joined = given.filter((one) => one !== undefined && one !== "").join(" ");
  return joined === "" ? undefined : joined;
}

/**
 * **One option**: its `RowLabel` is its name and its `RowDescription` its description, to a
 * reader too, with its `RowBelow` and a `RowLead` that says something after it (#6075); its
 * `textValue` is what typing finds it by.
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
  const { tipped, hold } = useListRowTip(tip);
  const element = useSaysUnavailable(unavailable, ref);
  const { heard, describedBy } = useRowHeard();
  const render = useMemo(() => optionElement(describedBy), [describedBy]);
  return (
    <AriaListBoxItem
      {...mergeProps(rest, own.handlers, tipped.hover)}
      ref={element}
      render={render}
      className={className}
      data-scope="list"
      data-part="root"
      data-variant="row"
      data-size={size}
      data-unavailable={unavailable || undefined}
    >
      {(state) => (
        <InOption.Provider value={heard}>
          <RowHeard.Provider value={heard}>
            {hold(rowHolds(children, own, tipped)(state))}
          </RowHeard.Provider>
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
  extends Drawn, Omit<AriaGridListProps<T>, "className" | "style" | "renderEmptyState"> {
  /** What Esc on a row does, in place of clearing what is chosen (#5332): a list in a mode of
   *  its own leaves it. */
  onEscape?: () => void;
  /**
   * **A long list draws only the rows in sight** (#6013), as a long tree does (#2947): React
   * Aria's own `Virtualizer` lays the rows out one under another, each measured as it is drawn,
   * so the keyboard still walks every row, and the list is its own scroller, so its owner's
   * `className` gives it a height. For a list that may hold every chat of every open project.
   */
  drawnInPart?: boolean;
}

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
  onEscape,
  drawnInPart = false,
  ...rest
}: ListProps<T>) {
  const own = useCollectionElement(keys, ref);
  const [laid, setLaid] = useState<Laid>(UNLAID);
  // A list drawn in part reads its room between rows and around them from its own CSS, as it is
  // drawn, so the layout places rows where the stylesheet would.
  const measure = useCallback((element: HTMLElement | null) => {
    if (element === null) return;
    const read = laidOf(element);
    setLaid((was) => (was.gap === read.gap && was.padding === read.padding ? was : read));
  }, []);
  const measured = useMemo(() => mergeRefs<HTMLDivElement>(measure, own), [measure, own]);
  const chooses = rest.selectionMode !== undefined && rest.selectionMode !== "none";
  const [folds] = useState(() => new Map<string, ((open: boolean) => void) | undefined>());
  const foldKeys = useFoldKeys(folds, rest.disallowTypeAhead !== true);
  const { direction } = useLocale();
  const onKeyDownCapture = (event: ReactKeyboardEvent<HTMLElement>) => {
    // **A text box in a row keeps its caret's arrows** (#3123, `rowKeys.ts`): the browser moves
    // its caret, and the row never walks its controls with them.
    if (keptOnTheWayDown(event)) {
      event.stopPropagation();
      return;
    }
    if (movedInARadioGroup(event, direction === "rtl")) return;
    if (
      onEscape !== undefined &&
      event.key === "Escape" &&
      !isComposing(event) &&
      // Esc in a row's text box gives the keyboard back to its row first.
      !isEscapeInABox(event)
    ) {
      event.preventDefault();
      event.stopPropagation();
      onEscape();
      return;
    }
    foldKeys(event);
  };
  return (
    // An element around the list that draws nothing, so → and ← on a row that folds are heard
    // before React Aria's row hears them (`useFoldKeys`).
    <div data-scope="list" data-part="keys" onKeyDownCapture={onKeyDownCapture}>
      <ListFolds.Provider value={folds}>
        <ListChooses.Provider value={chooses}>
          <RowScope.Provider value="list">
            <ListSize.Provider value={size}>
              <InPart drawnInPart={drawnInPart} laid={laid}>
                <AriaGridList
                  {...rest}
                  // A list that chooses never chooses a fold's row, and still reaches and runs it.
                  disabledBehavior={rest.disabledBehavior ?? (chooses ? "selection" : undefined)}
                  // A row that folds runs only its fold (`Fold.tsx`).
                  onAction={
                    onAction === undefined
                      ? undefined
                      : (key) => {
                          if (!folds.has(String(key))) onAction(key);
                        }
                  }
                  ref={drawnInPart ? measured : own}
                  className={className}
                  data-scope="list"
                  data-part="body"
                  data-size={size}
                  renderEmptyState={empty === undefined ? undefined : () => empty}
                />
              </InPart>
            </ListSize.Provider>
          </RowScope.Provider>
        </ListChooses.Provider>
      </ListFolds.Provider>
    </div>
  );
}

/** The room a list drawn in part keeps between its rows and around them, in px. */
interface Laid {
  gap: number;
  padding: number;
}

/** What a list is taken to keep before it is first drawn, and in a document with no layout. */
const UNLAID: Laid = { gap: 0, padding: 0 };

/** The room `element`, a list's body, keeps as its stylesheet draws it. */
function laidOf(element: HTMLElement): Laid {
  const style = getComputedStyle(element);
  const px = (value: string) => Number.parseFloat(value) || 0;
  return { gap: px(style.rowGap), padding: px(style.paddingTop) };
}

/** A row's height before it is measured: a two-line card's. */
const ROW_GUESS = 48;

/** **The list drawn in part, or whole**: in React Aria's `Virtualizer` with its list layout,
 *  each row measured as it is drawn, or as it is. */
function InPart({
  drawnInPart,
  laid,
  children,
}: {
  drawnInPart: boolean;
  laid: Laid;
  children: ReactNode;
}) {
  const options = useMemo(
    () => ({ estimatedRowHeight: ROW_GUESS, gap: laid.gap, padding: laid.padding }),
    [laid],
  );
  if (!drawnInPart) return children;
  return (
    <Virtualizer layout={ListLayout} layoutOptions={options}>
      {children}
    </Virtualizer>
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
  const { tipped, hold } = useListRowTip(tip);
  const { heard, describedBy } = useRowHeard();
  const render = useMemo(() => rowElement({ heard: describedBy }), [describedBy]);
  return (
    <AriaGridListItem
      {...mergeProps(rest, own.handlers, tipped.hover)}
      render={render}
      className={className}
      data-scope="list"
      data-part="root"
      data-variant={variant}
      data-size={size}
    >
      {(state) => (
        <RowHeard.Provider value={heard}>
          {hold(rowHolds(children, own, tipped)(state))}
        </RowHeard.Provider>
      )}
    </AriaGridListItem>
  );
}

export interface ListRowDrawnProps<T extends object> extends Omit<
  AriaGridListItemProps<T>,
  "className" | "style" | "children"
> {
  /** What it draws: another family's parts, which are its look. */
  children: ReactNode;
  /**
   * **Heard after each drawing of the row**, once what it draws is in the document. React Aria
   * draws a list's rows apart from the components that make them, a commit later, so an effect
   * of the component that makes a row would read the row as it was. What has to read the row as
   * it is drawn (where its answers stand, #3167) is told here.
   */
  onDrawn?: () => void;
  /** The other family's classes (`parted`): never a look of the screen's. */
  className?: string;
  /** Its name and description, by its own parts' ids, in place of its `textValue`. */
  "aria-labelledby"?: string;
  "aria-describedby"?: string;
  ref?: Ref<HTMLDivElement>;
  /** That family's `data-scope`, `data-part` and `data-variant`, as they are. */
  [data: `data-${string}`]: string | number | boolean | undefined;
}

/**
 * **A row another family draws** (#3123): a message of the Inbox's views (`MessageRow`), whose
 * row is its own root. The list gives it a row's role, its keyboard (↑ and ↓ to the rows above
 * and below, ← and → through its own controls, a text box in it keeping its own keys,
 * `rowKeys.ts`) and React Aria's state attributes (`data-hovered`, `data-focus-visible`); it
 * draws nothing, and writes no part of its own: the parts on it are the other family's, and so
 * is its look, its ring included. Its cell is drawn `display: contents`, so what it holds is laid
 * out as the other family lays out its root's children.
 */
export function ListRowDrawn<T extends object>({
  onDrawn,
  children,
  "aria-labelledby": labelledBy,
  "aria-describedby": describedBy,
  ...props
}: ListRowDrawnProps<T>) {
  return (
    <AriaGridListItem
      {...props}
      // **Named and described by its own parts**: React Aria hands a row neither, and names it
      // by its `textValue`; its element is drawn here with both, where they win over that name,
      // so a row is heard as its words say it (an ask by what it asks, then who asks).
      render={rowElement({ labelledBy, describedBy })}
    >
      {children}
      {onDrawn !== undefined && <Drawn then={onDrawn} />}
    </AriaGridListItem>
  );
}

/**
 * **A row's element, drawn by the family** (React Aria's `render`): named and described by the
 * ids given, where any are, and **the list's own row once anything in it takes the keyboard**
 * (#3123). React Aria makes a row the list's current one when the row itself is focused, or a
 * control in it by the pointer; focused any other way (a script moving the keyboard into a form
 * just opened, a screen reader's cursor) a control in a row the list had not reached left the
 * list on its first row, which then took the keyboard back a frame later. So the row a control is
 * focused in is told to the list first, before React Aria hears the focus. And **a Tab into the
 * list lands on a row, never on a control in it** (#6033, `backToTheRow`).
 */
function rowElement({
  labelledBy,
  describedBy,
  heard,
}: {
  labelledBy?: string;
  /** Its description whole, in place of React Aria's (`ListRowDrawn`). */
  describedBy?: string;
  /** What its parts hand it (`useRowHeard`), before what React Aria adds (#6101); for a row
   *  whose `describedBy` is not given. */
  heard?: string;
} = {}) {
  return (element: JSX.IntrinsicElements["div"], { id, state }: GridListItemRenderProps) => (
    <div
      {...element}
      aria-labelledby={labelledBy ?? element["aria-labelledby"]}
      aria-describedby={describedBy ?? idsOf(heard, element["aria-describedby"])}
      onFocus={(event) => {
        const manager = state.selectionManager;
        // A move a key makes is React Aria's own, already placed: its arrows and its Tab out of
        // the list (which passes the keyboard through the list's last control). Coming in from
        // outside is React Aria's too, but for a Tab whose control is in its own row (#6033).
        const from = event.relatedTarget;
        const list = event.currentTarget.closest('[role="grid"]');
        const fromWithin = from === null || (from instanceof Node && list?.contains(from) === true);
        if (!keyHeardNow && fromWithin && event.target !== event.currentTarget)
          if (id !== undefined && manager.focusedKey !== id) manager.setFocusedKey(id);
        if (tabHeardNow && !fromWithin && event.target !== event.currentTarget)
          backToTheRow(event.target, event.currentTarget, id, manager);
        element.onFocus?.(event);
      }}
      // A select or a combobox in it hears its own ↑ and ↓, and opens (#5993, `rowKeys.ts`).
      onKeyDownCapture={(event) => {
        if (!keepsItsArrows(event)) element.onKeyDownCapture?.(event);
      }}
      // Tab from a control in it goes on to its next control, which the list would skip to
      // leave itself (#6071, `rowKeys.ts`). And the ↑ or ↓ a select or a combobox opened with
      // goes no further (#6091, `keepsItsArrows`).
      onKeyDown={(event) => {
        if (movesWithinItsRow(event) || keepsItsArrows(event)) event.stopPropagation();
        element.onKeyDown?.(event);
      }}
    />
  );
}

/**
 * **Whether a key is being heard now**, anywhere in the window (`rowElement`): set as a key
 * comes down, on the window's capture phase, before anything hears it, and let go in the task
 * after it, once it has been heard everywhere and its own move (Tab's) is made.
 */
let keyHeardNow = false;
/** **Whether that key is Tab**, with Shift or without, whose move the browser makes next. */
let tabHeardNow = false;
if (typeof window !== "undefined")
  window.addEventListener(
    "keydown",
    (event) => {
      keyHeardNow = true;
      tabHeardNow = event.key === "Tab";
      // A task, never a microtask: for a key the person pressed, the browser runs microtasks
      // after each listener, so one would let go before React has heard the key at all.
      setTimeout(() => {
        keyHeardNow = false;
        tabHeardNow = false;
      }, 0);
    },
    { capture: true },
  );

/**
 * **Tab into a list lands on a row, never on a control in it** (#6033, the WAI-ARIA grid's one
 * Tab stop): Shift+Tab from below lands, the browser's way, on the list's last control, and
 * React Aria moves the keyboard to the row it keeps (the one the keyboard left, or the last) only
 * when that control is in another row. So once React Aria has heard the focus and chosen its row,
 * a `control` that still has the keyboard is in that row, `row`, and gives the keyboard to it;
 * → reaches the row's controls again from there.
 */
function backToTheRow(
  control: Element,
  row: HTMLElement,
  key: Key | undefined,
  manager: GridListItemRenderProps["state"]["selectionManager"],
) {
  queueMicrotask(() => {
    if (key === undefined || manager.focusedKey !== key) return;
    // React Aria has scrolled the row into view already, as it does on a Tab into its list.
    if (control.ownerDocument.activeElement === control) row.focus({ preventScroll: true });
  });
}

/** Runs `then` after each commit of the row it stands in, as part of the row. */
function Drawn({ then }: { then: () => void }) {
  useLayoutEffect(() => then());
  return null;
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
  ...data
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
  /** `data-*` attributes reach the element as they are (the chat a group is, #3123). */
  [attribute: `data-${string}`]: string | number | boolean | undefined;
}) {
  return (
    <AriaGridListSection<T>
      {...data}
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
