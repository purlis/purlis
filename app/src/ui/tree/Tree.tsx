// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The tree family** (#2911, spec #2897, ADR 0104): every tree in the window, on React Aria's
 * `Tree`, wrapped here once. A screen imports these and never `react-aria-components`
 * (`eslint.config.js`), and no screen writes a tree's role, class or scope by hand
 * (`purlis/tree-family`).
 *
 * - **A row is the list's row** (`ui/list/Row.tsx`): its `lead`, `label`, `description` and
 *   `trail`, in the tree's own scope, so a theme's `tree.label.text` reaches a tree's words and
 *   never a list's. The row is the family's root, `data-scope="tree" data-part="root"` with its
 *   `data-size`, and React Aria's state attributes (`data-hovered`, `data-selected`,
 *   `data-focus-visible`, `data-expanded`, `data-level`). `tree.css` draws the family default.
 * - **The WAI-ARIA tree's keys** (APG "Tree View"): one Tab stop, the row last focused; ↑ ↓,
 *   Home and End; → opens a closed row and moves into an open one's first child; ← closes an
 *   open row and moves from a closed one to its parent; typing finds a row by its `textValue`;
 *   Enter runs the row's action. React Aria draws it as a `treegrid` of rows (D-2911-T-1), the
 *   one tree it builds, whose rows may hold their own buttons.
 * - **The twist** (`twist`): a row with rows under it draws its chevron, the twist, in the step
 *   before its words (`"lead"`, R129: a file tree's ▸/▾), or as a toggle of its own at its end
 *   (`"trail"`, #2799 rule 1: a row that is itself a page, whose twist must not be its press).
 *   The twist is a button a pointer and a reader's own cursor press, never a Tab stop, and the
 *   arrows never land on it; a mark in the quiet tier (R129). A tree whose rows never fold (`fixed`) draws none.
 * - **Folders read when opened** (`expandedKeys`, `onExpandedChange`): the tree says which rows
 *   are open and the owner reads what is under one when it opens; while it reads, the row under
 *   it is a `TreeLine` that says so.
 * - **Indented in steps** (R129, 14 px a level at the default text size), each level above
 *   drawing one straight guide, at most four steps deep (`--tree-depth`, set on each row).
 * - **Drag with a way that is not a drag** (`draggable`): a row that `carries` something is
 *   dragged out with it, by the pointer and by the keyboard (its drag button, which a reader
 *   names); whatever a drag does, a row's menu or a button does too.
 * - **Rows its owner chooses** (`chosen`, and `choosesMany`): the file a tab shows, or a branch's
 *   files picked to hand a chat at once (#1151), are said and drawn as chosen while a press on a
 *   row still runs it (opens the file), which React Aria's own choosing, where a press chooses,
 *   would not let it.
 */
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  type CSSProperties,
  type KeyboardEvent,
  type ReactNode,
  type Ref,
} from "react";
import {
  Button as AriaButton,
  ButtonContext as AriaButtonContext,
  Collection,
  Tree as AriaTree,
  TreeHeader as AriaTreeHeader,
  TreeItem as AriaTreeItem,
  TreeItemContent as AriaTreeItemContent,
  TreeLoadMoreItem as AriaTreeLoadMoreItem,
  TreeSection as AriaTreeSection,
  useDragAndDrop,
  useSlottedContext,
  type Key,
  type PressEvent,
  type Selection,
  type TreeItemContentRenderProps,
  type TreeProps as AriaTreeProps,
} from "react-aria-components";
import { mergeProps, mergeRefs, useLocale } from "react-aria";
import { ChevronRight } from "lucide-react";
import { rowHolds, useCollectionElement, useRowMenu, type RowMenu } from "../list/List";
import { RowScope, type RowSize } from "../list/Row";
import { useRowTip } from "../list/RowTip";
import type { Carried } from "../list/drag";
import type { Tip } from "../tooltip/Tooltip";

export type { Key, PressEvent, Selection };
export { Collection };

/** Where a row's twist is drawn: in the step before its words, or as a toggle at its end. */
export type TwistAt = "lead" | "trail";

/** What every row of a tree is drawn with, which the tree hands them. */
interface Drawing {
  size: RowSize;
  twist: TwistAt;
  fixed: boolean;
  stopAt?: Key;
}

const TreeDrawing = createContext<Drawing>({ size: "md", twist: "lead", fixed: false });

export interface TreeProps<T extends object> extends Omit<
  AriaTreeProps<T>,
  "className" | "style" | "renderEmptyState"
> {
  /** Every row's size: `sm` (24 px) in a side view, `md` (28) the default. */
  size?: RowSize;
  /** Where a row with rows under it draws its twist (the default: in the step before it). */
  twist?: TwistAt;
  /**
   * **Nothing folds** (the Changes view, ADR 0038): every row is drawn open and nothing in the
   * tree can be pressed; ← moves from a row to its parent and → into its first child. The
   * owner names every row with rows under it in `expandedKeys`.
   */
  fixed?: boolean;
  /** A place for the tree in its view: margin, height, flex, grid area. Never a look. */
  className?: string;
  style?: CSSProperties;
  /** What the tree says where it has no rows: the empty state, or a line. */
  empty?: ReactNode;
  /** The keys the tree answers to beside the arrows, said to a reader. */
  "aria-keyshortcuts"?: string;
  /** Its rows are dragged out with what each `carries` (`ui/list/drag.ts`). */
  draggable?: boolean;
  /**
   * **Its rows are as wide as their words** where the view scrolls sideways (the Explorer, the
   * Changes view): a row is never narrower than its one line, so its pill and its ring reach
   * the end of a long name (#1796). By default a row is as wide as the tree and cuts its words.
   */
  fit?: "tree" | "content";
  /** Its owner chooses several of its rows at once (`chosen` on a row). */
  choosesMany?: boolean;
  /**
   * **The row the tree's one Tab stop is on** until the keyboard has been in it (the place the
   * next chat starts, `current`), as a tree whose press chooses stops on its choice.
   */
  stopAt?: Key;
  /** **A key the owner takes before the tree's own**: true where it took it (Shift and the
   *  arrows picking rows). */
  onRowKey?: (event: KeyboardEvent<HTMLElement>) => boolean;
  ref?: Ref<HTMLDivElement>;
  /** `data-*` attributes reach the element as they are. */
  [data: `data-${string}`]: string | number | boolean | undefined;
}

/** A row's element and what a key on it reads: its level and whether it is open. */
function rowOf(target: EventTarget | null): HTMLElement | null {
  return target instanceof HTMLElement && target.getAttribute("role") === "row" ? target : null;
}

function levelOf(row: Element): number {
  return Number(row.getAttribute("aria-level") ?? "1");
}

/** The rows of the tree a row is in, in their order. */
function rowsBeside(row: HTMLElement): HTMLElement[] {
  const tree = row.closest('[role="treegrid"]');
  return tree === null ? [] : [...tree.querySelectorAll<HTMLElement>('[role="row"]')];
}

/** The first row under an open `row`, or none. */
function firstChildOf(row: HTMLElement): HTMLElement | undefined {
  const rows = rowsBeside(row);
  const next = rows[rows.indexOf(row) + 1];
  return next !== undefined && levelOf(next) === levelOf(row) + 1 ? next : undefined;
}

/** The row `row` is under, or none at the top. */
function parentOf(row: HTMLElement): HTMLElement | undefined {
  const rows = rowsBeside(row);
  const level = levelOf(row);
  for (let at = rows.indexOf(row) - 1; at >= 0; at -= 1)
    if (levelOf(rows[at]) < level) return rows[at];
  return undefined;
}

/**
 * **← and → as the WAI-ARIA tree has them**, where React Aria's grid would walk into a row's
 * controls instead (its twist among them): heard before React Aria's own row (a capture handler
 * on an element around the tree runs first).
 *
 * - → on an open row moves to its first child; on a closed one React Aria opens it.
 * - ← on a row at the top that is closed, or has no rows under it, does nothing; anywhere else
 *   React Aria closes it or moves to its parent.
 * - In a tree whose rows never fold, ← on an open row moves to its parent.
 *
 * A key on a row's own control (focus inside the row) is React Aria's: it walks the controls.
 */
function useTreeKeys(
  fixed: boolean,
  owner: ((event: KeyboardEvent<HTMLElement>) => boolean) | undefined,
) {
  const { direction } = useLocale();
  return (event: KeyboardEvent<HTMLElement>) => {
    // A key from a row's menu or another portal React holds in the tree is not the tree's.
    if (!event.currentTarget.contains(event.target as Node)) return;
    if (owner?.(event)) {
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    const row = rowOf(event.target);
    if (row === null) return;
    // A ← or → with a modifier is not the tree's: React Aria would open or close the row.
    if (
      (event.altKey || event.ctrlKey || event.metaKey) &&
      (event.key === "ArrowLeft" || event.key === "ArrowRight")
    ) {
      event.stopPropagation();
      return;
    }
    if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
    const into = direction === "rtl" ? "ArrowLeft" : "ArrowRight";
    const out = direction === "rtl" ? "ArrowRight" : "ArrowLeft";
    const open = row.getAttribute("aria-expanded") === "true";
    const go = (to: HTMLElement | undefined) => {
      event.preventDefault();
      event.stopPropagation();
      to?.focus();
    };
    if (event.key === into && open) go(firstChildOf(row));
    else if (event.key === out) {
      const parent = parentOf(row);
      if (parent === undefined && (!open || fixed)) go(undefined);
      else if (open && fixed) go(parent);
    }
  };
}

/** What a row is to React Aria: what it carries when it is dragged. */
interface Value {
  carries?: Carried;
}

/**
 * **An attribute React Aria does not write, written on the element**: a tree whose owner chooses
 * several rows says it is multiselectable, and a chosen row that it is selected, as a tree whose
 * press chooses says them. Nothing where `value` is undefined.
 */
function useSaid<E extends HTMLElement>(
  name: string,
  value: string | undefined,
  given: Ref<E> | undefined,
) {
  // It takes back only what it wrote, which it marks: React Aria writes the same attributes on
  // a tree whose press chooses.
  const says = useCallback(
    (element: E | null) => {
      if (element === null) return;
      const mark = `data-said-${name}`;
      if (value !== undefined) {
        element.setAttribute(name, value);
        element.setAttribute(mark, "");
      } else if (element.hasAttribute(mark)) {
        element.removeAttribute(name);
        element.removeAttribute(mark);
      }
    },
    [name, value],
  );
  return useMemo(() => mergeRefs<E>(says, given ?? null), [says, given]);
}

/**
 * **A tree**: rows that may hold rows under them, which fold open and closed (or never, `fixed`),
 * may be chosen (`selectionMode`), and run something (`onAction`).
 */
export function Tree<T extends object>({
  size = "md",
  twist = "lead",
  fixed = false,
  className,
  style,
  empty,
  "aria-keyshortcuts": keys,
  draggable = false,
  fit = "tree",
  choosesMany = false,
  stopAt,
  onRowKey,
  ref,
  onExpandedChange,
  ...rest
}: TreeProps<T>) {
  const said = useSaid("aria-multiselectable", choosesMany ? "true" : undefined, ref);
  const own = useCollectionElement(keys, said);
  const onKeyDownCapture = useTreeKeys(fixed, onRowKey);
  const drawing = useMemo(() => ({ size, twist, fixed, stopAt }), [size, twist, fixed, stopAt]);
  // What a dragged row carries is its own (`carries`, its value to React Aria).
  const drag = useDragAndDrop<Value>({
    getItems: (_keys, items) => items.flatMap((item) => (item?.carries ? [item.carries] : [])),
    getAllowedDropOperations: () => ["copy"],
  }).dragAndDropHooks;
  return (
    // An element around the tree that draws nothing, so its keys are heard before React Aria's
    // row hears them (`useTreeKeys`).
    <div data-scope="tree" data-part="keys" onKeyDownCapture={onKeyDownCapture}>
      <RowScope.Provider value="tree">
        <TreeDrawing.Provider value={drawing}>
          <AriaTree
            {...rest}
            // React Aria takes a tree's drag hooks once: a tree that starts or stops dragging
            // (the plane root's files after a workspace's) is another tree.
            key={draggable ? "drags" : "still"}
            // A tree that never folds keeps what its owner says is open.
            onExpandedChange={fixed ? undefined : onExpandedChange}
            ref={own}
            dragAndDropHooks={draggable ? drag : undefined}
            className={className}
            style={style}
            data-scope="tree"
            data-part="body"
            data-size={size}
            data-twist={fixed ? "none" : twist}
            data-fit={fit === "content" ? "content" : undefined}
            renderEmptyState={empty === undefined ? undefined : () => empty}
          />
        </TreeDrawing.Provider>
      </RowScope.Provider>
    </div>
  );
}

/**
 * **The twist**: the chevron that opens and closes the row's rows, pointing at its words while
 * closed and down while open. React Aria's own toggle (its `chevron` slot), named "Expand ‹the
 * row›" or "Collapse ‹the row›"; never a Tab stop, and the arrows do not land on it.
 */
function Twist() {
  return (
    <AriaButton slot="chevron" data-scope="tree" data-part="twist">
      <ChevronRight aria-hidden="true" />
    </AriaButton>
  );
}

/**
 * **The tree's Tab stop, on this row** (`stopAt`): while the keyboard is not in the tree, its
 * stop moves to the row, so Tab lands where the owner says; the keyboard's own place wins once
 * it has been in.
 */
function TakesTheStop({ at, state }: { at: Key; state: TreeItemContentRenderProps["state"] }) {
  const manager = state.selectionManager;
  // Once for each row it is told to stop on: a redraw never takes the stop back from where the
  // keyboard left it.
  const stopped = useRef<Key>(undefined);
  useEffect(() => {
    if (stopped.current === at) return;
    stopped.current = at;
    if (!manager.isFocused && manager.focusedKey !== at) manager.setFocusedKey(at);
  }, [manager, at]);
  return null;
}

/**
 * **No drag button on a row** (#2911 review, B4): the pointer drags the row itself, and the
 * keyboard's way is the row's menu (*Add to a chat's context*), since the window's drop targets
 * are no React Aria drop targets a keyboard drag could reach. React Aria looks for a drag button
 * and warns of a row with none; the slot's ref is held by an element that draws nothing and
 * takes no focus, as {@link HeldOpen} holds the twist's, so → on a row never lands on one.
 */
function NoDrag() {
  const slot = useSlottedContext(AriaButtonContext, "drag");
  return <span ref={slot?.ref as Ref<HTMLSpanElement>} data-part="no-drag" hidden />;
}

/**
 * **A fixed tree's row with rows under it**: it is open and says so, and has no twist to press.
 * React Aria looks for a row's toggle and warns of a row with none; the slot's ref is held by an
 * element that draws nothing and takes no pointer or key.
 */
function HeldOpen() {
  const slot = useSlottedContext(AriaButtonContext, "chevron");
  return <span ref={slot?.ref as Ref<HTMLSpanElement>} data-part="held-open" hidden />;
}

export interface TreeRowProps {
  /** Its key among the tree's rows. */
  id: Key;
  /** What typing finds it by and what a reader hears first. */
  textValue: string;
  /** What it holds: the row's slots (`RowLead`, `RowLabel`, `RowDescription`, `RowTrail`). */
  content: ReactNode | ((state: TreeItemContentRenderProps) => ReactNode);
  /** The rows under it, which its twist opens and closes. */
  children?: ReactNode;
  /** It has rows under it that are not drawn yet (a folder not read): it draws its twist. */
  hasChildItems?: boolean;
  /** Run when it is pressed, or Enter is pressed on it. */
  onAction?: () => void;
  /** It cannot be used. */
  isDisabled?: boolean;
  /**
   * **It cannot be run now, and says so** (`aria-disabled`, the muted words): it keeps the
   * keyboard, so its tip, which says why, is read (a file purlis will not open).
   */
  unavailable?: boolean;
  /** **Chosen by its owner**: said (`aria-selected`) and drawn as chosen; `false` for a row that
   *  can be chosen and is not, nothing for one that cannot be. */
  chosen?: boolean;
  /**
   * **The current one of its kind** (`aria-current`), drawn as a chosen row is: the place the
   * next chat starts, which is a place and not a choice the arrows move (ADR 0036).
   */
  current?: boolean;
  /** Its rows are always drawn: it never folds, and draws no twist (a workspace's own row). */
  alwaysOpen?: boolean;
  /**
   * **What it is beside its name, to a reader** (`aria-description`): its marks' words, heard
   * after its name (`textValue`), which stays what typing finds it by.
   */
  description?: string;
  /** What it carries when it is dragged out of a `draggable` tree. */
  carries?: Carried;
  /** Its `tip` shows to the pointer alone (a file's path), never on the keyboard. */
  tipToPointer?: boolean;
  /** What is under it is being read (`aria-busy`), while its `TreePending` stands there. */
  busy?: boolean;
  /** A press on it begins: its modifier keys, before its action runs. */
  onPressStart?: (event: PressEvent) => void;
  /** Its context menu: a right-click, Shift+F10 or the menu key opens it. */
  menu?: RowMenu;
  /** What it says that its words do not, beside it: under the pointer, and while the keyboard
   *  is on it (the shared tooltip, `ui/list/RowTip.tsx`). */
  tip?: Tip;
  /** A name for a reader where its `textValue` is not the whole of it. */
  "aria-label"?: string;
  /** A place for the row in its tree: never a look. */
  className?: string;
  style?: CSSProperties;
  ref?: Ref<HTMLDivElement>;
  /** `data-*` attributes reach the element as they are. */
  [data: `data-${string}`]: string | number | boolean | undefined;
}

/** **One row of a tree**, with the rows under it as its children. */
export function TreeRow<T extends object>({
  content,
  children,
  menu,
  tip,
  unavailable = false,
  chosen,
  tipToPointer = false,
  busy = false,
  current = false,
  alwaysOpen = false,
  description,
  carries,
  className,
  style,
  ref,
  ...rest
}: TreeRowProps) {
  const { size, twist, fixed, stopAt } = useContext(TreeDrawing);
  const owned = useRowMenu(menu);
  const tipped = useRowTip(tip, tipToPointer);
  const selected = useSaid("aria-selected", chosen === undefined ? undefined : String(chosen), ref);
  const shown = useSaid("data-selected", chosen || current ? "true" : undefined, selected);
  const now = useSaid("aria-current", current ? "true" : undefined, shown);
  const told = useSaid("aria-description", description || undefined, now);
  const reading = useSaid("aria-busy", busy ? "true" : undefined, told);
  const element = useSaid("aria-disabled", unavailable ? "true" : undefined, reading);
  return (
    <AriaTreeItem<T>
      {...mergeProps(rest, owned.handlers, tipped.hover)}
      ref={element}
      value={{ carries } as T}
      data-unavailable={unavailable || undefined}
      className={className}
      // How deep it is, which its steps and guides are drawn from (`tree.css`).
      style={({ level }) => ({ ...style, "--tree-depth": level - 1 }) as CSSProperties}
      data-scope="tree"
      data-part="root"
      data-size={size}
    >
      <AriaTreeItemContent>
        {(state) => {
          const toggle = state.hasChildItems ? (
            fixed || alwaysOpen ? (
              <HeldOpen />
            ) : (
              <Twist />
            )
          ) : null;
          return (
            <>
              {state.id === stopAt && <TakesTheStop at={state.id} state={state.state} />}
              {twist === "lead" && toggle}
              {rowHolds(content, owned.menu, tipped)(state)}
              {twist === "trail" && toggle}
              {state.allowsDragging && <NoDrag />}
            </>
          );
        }}
      </AriaTreeItemContent>
      {children ??
        // **A row whose rows are not drawn yet still has rows under it** (a folder not read):
        // React Aria knows a row has children by the rows in its collection, so it holds one
        // that is never drawn while the row is closed, and the owner's once it opens.
        (rest.hasChildItems && (
          <AriaTreeItem id={`${String(rest.id)}\u0000unread`} textValue="">
            {null}
          </AriaTreeItem>
        ))}
    </AriaTreeItem>
  );
}

/**
 * **A line of a tree that is not a row to act on**: what a folder still being read says, why
 * it could not be, how many more it holds than are drawn. It is a row to the keyboard, so it is
 * read in its place, and it runs nothing.
 */
export function TreeLine({
  id,
  textValue,
  children,
  className,
  ...rest
}: {
  id: Key;
  textValue: string;
  children: ReactNode;
  className?: string;
  /** `data-*` attributes reach the element as they are. */
  [data: `data-${string}`]: string | number | boolean | undefined;
}) {
  return (
    <TreeRow
      {...rest}
      id={id}
      textValue={textValue}
      className={className}
      data-line=""
      content={children}
    />
  );
}

/**
 * **What stands under a row while its rows are read** ("Reading…", "Asking git…"): React Aria's
 * loading row, drawn in its place and said, and passed by the arrows, so the keyboard never
 * rests on a line that goes away under it. Its row says `busy` meanwhile. A line that lasts
 * (why a folder could not be read, "Nothing here") is a `TreeLine`, which the arrows read.
 */
export function TreePending({
  children,
  className,
  ...rest
}: {
  children: ReactNode;
  className?: string;
  /** `data-*` attributes reach the element as they are. */
  [data: `data-${string}`]: string | number | boolean | undefined;
}) {
  const { size } = useContext(TreeDrawing);
  return (
    <AriaTreeLoadMoreItem
      {...rest}
      isLoading
      className={className}
      data-scope="tree"
      data-part="pending"
      data-size={size}
      style={({ level }) => ({ "--tree-depth": level - 1 }) as CSSProperties}
    >
      {children}
    </AriaTreeLoadMoreItem>
  );
}

/** **A group of rows under its name**: the name is the section's `header` part. */
export function TreeSection<T extends object>({
  title,
  named = false,
  className,
  children,
  "aria-label": label,
  id,
}: {
  /** The group's name, drawn over its rows. */
  title?: ReactNode;
  /** The name is data (a project's, a day's), drawn as it is written; else purlis's own label. */
  named?: boolean;
  className?: string;
  children: ReactNode;
  "aria-label"?: string;
  id?: Key;
}) {
  return (
    <AriaTreeSection<T>
      id={id}
      className={className}
      aria-label={label}
      data-scope="tree"
      data-part="section"
    >
      {title !== undefined && (
        <AriaTreeHeader data-scope="tree" data-part="header" data-named={named || undefined}>
          {title}
        </AriaTreeHeader>
      )}
      {children}
    </AriaTreeSection>
  );
}
