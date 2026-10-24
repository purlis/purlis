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
 * - **Drag with a way that is not a drag**: `dragAndDropHooks` (`ui/list/drag.ts`); whatever a
 *   drag does, a row's menu or a button does too.
 */
import {
  createContext,
  useContext,
  useMemo,
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
  TreeSection as AriaTreeSection,
  useSlottedContext,
  type Key,
  type Selection,
  type TreeItemContentRenderProps,
  type TreeProps as AriaTreeProps,
} from "react-aria-components";
import { mergeProps, useLocale } from "react-aria";
import { ChevronRight } from "lucide-react";
import { rowHolds, useCollectionElement, useRowMenu, type RowMenu } from "../list/List";
import { RowScope, type RowSize } from "../list/Row";
import { useRowTip } from "../list/RowTip";
import type { Tip } from "../tooltip/Tooltip";

export type { Key, Selection };
export { Collection };

/** Where a row's twist is drawn: in the step before its words, or as a toggle at its end. */
export type TwistAt = "lead" | "trail";

/** What every row of a tree is drawn with, which the tree hands them. */
interface Drawing {
  size: RowSize;
  twist: TwistAt;
  fixed: boolean;
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
function useTreeKeys(fixed: boolean) {
  const { direction } = useLocale();
  return (event: KeyboardEvent<HTMLElement>) => {
    if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
    const row = rowOf(event.target);
    if (row === null) return;
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
  ref,
  onExpandedChange,
  ...rest
}: TreeProps<T>) {
  const own = useCollectionElement(keys, ref);
  const onKeyDownCapture = useTreeKeys(fixed);
  const drawing = useMemo(() => ({ size, twist, fixed }), [size, twist, fixed]);
  return (
    // An element around the tree that draws nothing, so its keys are heard before React Aria's
    // row hears them (`useTreeKeys`).
    <div data-scope="tree" data-part="keys" onKeyDownCapture={onKeyDownCapture}>
      <RowScope.Provider value="tree">
        <TreeDrawing.Provider value={drawing}>
          <AriaTree
            {...rest}
            // A tree that never folds keeps what its owner says is open.
            onExpandedChange={fixed ? undefined : onExpandedChange}
            ref={own}
            className={className}
            style={style}
            data-scope="tree"
            data-part="body"
            data-size={size}
            data-twist={fixed ? "none" : twist}
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
  className,
  style,
  ...rest
}: TreeRowProps) {
  const { size, twist, fixed } = useContext(TreeDrawing);
  const owned = useRowMenu(menu);
  const tipped = useRowTip(tip);
  return (
    <AriaTreeItem<T>
      {...mergeProps(rest, owned.handlers, tipped.hover)}
      className={className}
      // How deep it is, which its steps and guides are drawn from (`tree.css`).
      style={({ level }) => ({ ...style, "--tree-depth": level - 1 }) as CSSProperties}
      data-scope="tree"
      data-part="root"
      data-size={size}
    >
      <AriaTreeItemContent>
        {(state) => {
          const toggle = state.hasChildItems ? fixed ? <HeldOpen /> : <Twist /> : null;
          return (
            <>
              {twist === "lead" && toggle}
              {rowHolds(content, owned.menu, tipped)(state)}
              {twist === "trail" && toggle}
            </>
          );
        }}
      </AriaTreeItemContent>
      {children}
    </AriaTreeItem>
  );
}

/**
 * **A line of a tree that is not a row to act on**: what a folder still being read says, why
 * it could not be, how many more it holds than are drawn. It is a row to the keyboard, so it is
 * read in its place, and it runs nothing.
 */
export function TreeLine<T extends object>({
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
    <TreeRow<T>
      {...rest}
      id={id}
      textValue={textValue}
      className={className}
      data-line=""
      content={children}
    />
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
