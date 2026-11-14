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
 *   Enter runs the row's action. On an open row with controls of its own, → moves into them, as
 *   the APG treegrid's does (#3198). React Aria draws it as a `treegrid` of rows (D-2911-T-1), the
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
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type ReactNode,
  type Ref,
  type RefCallback,
} from "react";
import {
  Button as AriaButton,
  ButtonContext as AriaButtonContext,
  Collection,
  ListLayout,
  Virtualizer,
  Tree as AriaTree,
  TreeHeader as AriaTreeHeader,
  TreeItem as AriaTreeItem,
  TreeItemContent as AriaTreeItemContent,
  TreeLoadMoreItem as AriaTreeLoadMoreItem,
  TreeSection as AriaTreeSection,
  useDragAndDrop,
  useSlottedContext,
  type HoverEvent,
  type Key,
  type PressEvent,
  type Selection,
  type TreeItemContentRenderProps,
  type TreeProps as AriaTreeProps,
} from "react-aria-components";
import { mergeProps, mergeRefs, useLocale } from "react-aria";
import { ChevronRight } from "lucide-react";
import {
  groupHeader,
  rowHolds,
  useCollectionElement,
  useRowMenu,
  type RowMenu,
} from "../list/List";
import {
  heardIds,
  RowHeard,
  RowScope,
  type HearsParts,
  type PartHeard,
  type RowSize,
  withoutPart,
  withPart,
} from "../list/Row";
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
   * **Its rows may start dragging while it is drawn** (#3532): the tree is handed React Aria's
   * drag hooks from its first draw, off until `draggable`, so it stays the same tree when its
   * drag turns on. A tree that is `draggable` from its first draw, or never, needs none.
   */
  mayDrag?: boolean;
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
  /**
   * **A long tree draws only the rows in sight** (#2947): React Aria's own `Virtualizer` lays the
   * open rows out one under another, each measured as it is drawn, and the tree is its own
   * scroller, so its owner's `className` gives it a height. For a tree that may hold a thousand
   * rows and more (the parts manifest); a tree whose folders the core caps draws whole.
   */
  drawnInPart?: boolean;
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

/**
 * **Whether a row holds controls of its own** (#3198): a button, a link or a field in it that is
 * not its twist, which the tree family's grid reaches by → from the row.
 */
function holdsControls(row: HTMLElement): boolean {
  return [
    ...row.querySelectorAll<HTMLElement>(
      'button, a[href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
    ),
  ].some((one) => one.closest('[data-part="twist"], [hidden]') === null && one.slot !== "chevron");
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
 * - → on an open row moves to its first child; on a closed one React Aria opens it. **An open
 *   row with controls of its own** (a chat's hand, its ⋯) is the APG treegrid's: → moves into
 *   its controls, as on a row with nothing under it, and ↓ goes on to its first child (#3198).
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
    if (event.key === into && open && !holdsControls(row)) go(firstChildOf(row));
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
 * **What a row is beside its name, said from inside it** (#4730): the ref of an element drawn in
 * a row's `content` writes `description` on the row as its `aria-description`, and takes it back
 * when the element goes.
 *
 * For words that change far more often than the row, such as a chat's live touch: the part that
 * says them is drawn again for them, and the row is not. A row drawn again is the tree drawn
 * again, every row of it, because React Aria builds its collection again for any row that is. A
 * row whose words come from here passes no `description` of its own.
 */
export function useRowDescription<E extends HTMLElement>(
  description: string | undefined,
): RefCallback<E> {
  const row = useRef<HTMLElement | null>(null);
  return useCallback(
    (element: E | null) => {
      // Its own mark, apart from `TreeRow`'s: each takes back only what it wrote.
      const mark = "data-said-inside-aria-description";
      const takeBack = (from: HTMLElement | null) => {
        if (from?.hasAttribute(mark)) {
          from.removeAttribute("aria-description");
          from.removeAttribute(mark);
        }
      };
      if (element === null) {
        takeBack(row.current);
        row.current = null;
        return;
      }
      row.current = element.closest<HTMLElement>('[role="row"]');
      if (description === undefined) takeBack(row.current);
      else {
        row.current?.setAttribute("aria-description", description);
        row.current?.setAttribute(mark, "");
      }
    },
    [description],
  );
}

/**
 * **What a tree row's parts are heard by** (#6123): the registry a list's row has (`RowHeard`,
 * #6101), its ids written on the row's element from here, its root, because React Aria's
 * `TreeItem` takes no `render` and filters `aria-describedby` out of its props. Its tooltip hands
 * its id here while it is up (`RowTip.tsx`), and nothing else in the window writes the attribute
 * (`describedBy.guard.test.ts`).
 *
 * Written on the element and not drawn: what a part hands it draws no row again, which would
 * draw the tree again (`useRowDescription`). It takes back only the ids it wrote, so one written
 * by anything else (React Aria's own, which it writes on no tree row today) stays.
 */
function useTreeRowHeard<E extends HTMLElement>(
  given: Ref<E> | undefined,
): { heard: HearsParts; ref: Ref<E> } {
  const [{ heard, attach }] = useState(() => treeRowHeard<E>());
  const ref = useMemo(() => mergeRefs<E>(attach, given ?? null), [attach, given]);
  return { heard, ref };
}

/** A tree row's registry and the ref that hands it the row's element, built once a row. */
function treeRowHeard<E extends HTMLElement>(): { heard: HearsParts; attach: RefCallback<E> } {
  let row: E | null = null;
  let parts: readonly PartHeard[] = [];
  let wrote: readonly string[] = [];
  /** The ids on `element` now, but for those this row wrote there. */
  const othersOn = (element: E) =>
    (element.getAttribute("aria-describedby") ?? "")
      .split(/\s+/)
      .filter((id) => id !== "" && !wrote.includes(id));
  const say = (element: E, ids: readonly string[]) => {
    if (ids.length === 0) element.removeAttribute("aria-describedby");
    else element.setAttribute("aria-describedby", ids.join(" "));
  };
  const write = () => {
    if (row === null) return;
    const ours = heardIds(parts);
    const others = othersOn(row);
    wrote = ours;
    say(row, [...others, ...ours]);
  };
  return {
    heard: {
      add(kind, id) {
        parts = withPart(parts, { kind, id });
        write();
        return () => {
          parts = withoutPart(parts, id);
          write();
        };
      },
    },
    // Handed the element again whenever a ref around it changes (`useSaid`'s), after `null`: what
    // it wrote is taken back from the element it leaves, and written afresh on the one it gets.
    attach(element) {
      if (element === null) {
        if (row !== null) say(row, othersOn(row));
        wrote = [];
      }
      row = element;
      write();
    },
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
  draggable = false,
  mayDrag = false,
  fit = "tree",
  choosesMany = false,
  stopAt,
  onRowKey,
  drawnInPart = false,
  ref,
  onExpandedChange,
  ...rest
}: TreeProps<T>) {
  const said = useSaid("aria-multiselectable", choosesMany ? "true" : undefined, ref);
  const own = useCollectionElement(keys, said);
  const onKeyDownCapture = useTreeKeys(fixed, onRowKey);
  const drawing = useMemo(() => ({ size, twist, fixed, stopAt }), [size, twist, fixed, stopAt]);
  const [laid, setLaid] = useState<Laid>(UNLAID);
  // A tree drawn in part reads its room between rows and around them from its own CSS, as it is
  // drawn, so the layout places rows where the stylesheet would.
  const measure = useCallback((element: HTMLElement | null) => {
    if (element === null) return;
    const read = laidOf(element);
    setLaid((was) => (was.gap === read.gap && was.padding === read.padding ? was : read));
  }, []);
  const measured = useMemo(() => mergeRefs<HTMLDivElement>(measure, own), [measure, own]);
  // What a dragged row carries is its own (`carries`, its value to React Aria). **A tree that
  // may drag is handed the hooks from its first draw, off until it drags** (#3532): React Aria
  // takes a tree's drag hooks once, so handing them over only as the drag turns on would make it
  // another tree, and the row the keyboard was on (the plane's files once the plane's read
  // lands) would leave the page with the focus. A tree that never drags is handed none, so its
  // rows run no drag hooks at all.
  const drag = useDragAndDrop<Value>({
    getItems: (_keys, items) => items.flatMap((item) => (item?.carries ? [item.carries] : [])),
    getAllowedDropOperations: () => ["copy"],
    isDisabled: !draggable,
  }).dragAndDropHooks;
  return (
    // An element around the tree that draws nothing, so its keys are heard before React Aria's
    // row hears them (`useTreeKeys`).
    <div data-scope="tree" data-part="keys" onKeyDownCapture={onKeyDownCapture}>
      <RowScope.Provider value="tree">
        <TreeDrawing.Provider value={drawing}>
          <InPart drawnInPart={drawnInPart} laid={laid}>
            <AriaTree
              {...rest}
              // A tree that never folds keeps what its owner says is open.
              onExpandedChange={fixed ? undefined : onExpandedChange}
              ref={drawnInPart ? measured : own}
              dragAndDropHooks={mayDrag || draggable ? drag : undefined}
              className={className}
              style={style}
              data-scope="tree"
              data-part="body"
              data-size={size}
              data-twist={fixed ? "none" : twist}
              data-fit={fit === "content" ? "content" : undefined}
              renderEmptyState={empty === undefined ? undefined : () => empty}
            />
          </InPart>
        </TreeDrawing.Provider>
      </RowScope.Provider>
    </div>
  );
}

/** The room a tree drawn in part keeps between its rows and around them, in px. */
interface Laid {
  gap: number;
  padding: number;
}

/** What a tree is taken to keep before it is first drawn, and in a document with no layout. */
const UNLAID: Laid = { gap: 0, padding: 0 };

/** The room `element`, a tree's body, keeps as its stylesheet draws it. */
function laidOf(element: HTMLElement): Laid {
  const style = getComputedStyle(element);
  const px = (value: string) => Number.parseFloat(value) || 0;
  return { gap: px(style.rowGap), padding: px(style.paddingTop) };
}

/** A row's height before it is measured: a control's, the default row's. */
const ROW_GUESS = 28;

/**
 * **The tree drawn in part, or whole**: in React Aria's `Virtualizer` with its list layout, each
 * row measured as it is drawn, or as it is.
 */
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
 * **The twist as a mark** (#3670): the same chevron, drawn by the same part, for a fold that is
 * not a tree's row (a list's section that folds, a run of rows folded to one line). It points at
 * the words while closed and down while open, from the `data-expanded` of the row it is in. It
 * is no button: the row it is in folds and unfolds, and says so with its own `aria-expanded`.
 */
export function TwistMark() {
  return (
    <span data-scope="tree" data-part="twist" aria-hidden="true">
      <ChevronRight aria-hidden="true" />
    </span>
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
 * A row of a tree that may drag holds one while its drag is off too (#3532): React Aria looks
 * for the button wherever the tree has the hooks.
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

/**
 * **The keyboard coming to a row and leaving it, heard on the row's element** (#3198): React Aria
 * draws the row and hands it no focus handler of its owner's, so an element that draws nothing
 * listens on the row it is in, for the row's own focus and not a control's in it.
 */
function FocusHeard({
  onFocus,
  onBlur,
}: {
  onFocus?: (event: FocusEvent) => void;
  onBlur?: (event: FocusEvent) => void;
}) {
  const heard = useRef({ onFocus, onBlur });
  useEffect(() => {
    heard.current = { onFocus, onBlur };
  });
  const at = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    const row = at.current?.closest<HTMLElement>('[role="row"]');
    if (row === null || row === undefined) return;
    const focus = (event: FocusEvent) => heard.current.onFocus?.(event);
    const blur = (event: FocusEvent) => heard.current.onBlur?.(event);
    row.addEventListener("focus", focus);
    row.addEventListener("blur", blur);
    return () => {
      row.removeEventListener("focus", focus);
      row.removeEventListener("blur", blur);
    };
  }, []);
  return <span ref={at} data-part="focus-heard" hidden />;
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
  /**
   * **The pointer comes to rest on it, and leaves** (#3198): what a row's own card is raised and
   * taken down by (a chat's, `ChatsSection.tsx`), as its `tip` is by the family. Not a touch.
   */
  onHoverStart?: (event: HoverEvent) => void;
  onHoverEnd?: (event: HoverEvent) => void;
  /**
   * **The keyboard comes to the row itself, and leaves it**: a row's own card again. Not a
   * control's in it. React Aria hands a row's element no focus handler of its owner's, so the
   * row hears its own (`FocusHeard`).
   */
  onFocus?: (event: FocusEvent) => void;
  onBlur?: (event: FocusEvent) => void;
  /** Its context menu: a right-click, Shift+F10 or the menu key opens it. */
  menu?: RowMenu;
  /** What it says that its words do not, beside it: under the pointer, and while the keyboard
   *  is on it (the shared tooltip, `ui/list/RowTip.tsx`). */
  tip?: Tip;
  /**
   * **The id of what its owner draws that describes it**, while it is drawn (a card it holds
   * up beside the row). Heard after its tooltip, through the row's registry, so the
   * row writes it with the rest and takes back only it as it goes (#6147).
   */
  describedBy?: string;
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
  onFocus,
  onBlur,
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
  describedBy,
  carries,
  className,
  style,
  ref,
  ...rest
}: TreeRowProps) {
  const { size, twist, fixed, stopAt } = useContext(TreeDrawing);
  const owned = useRowMenu(menu);
  const tipped = useRowTip(tip, tipToPointer);
  const { heard, ref: described } = useTreeRowHeard(ref);
  useEffect(
    () => (describedBy === undefined ? undefined : heard.add("owner", describedBy)),
    [heard, describedBy],
  );
  const selected = useSaid(
    "aria-selected",
    chosen === undefined ? undefined : String(chosen),
    described,
  );
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
            <RowHeard.Provider value={heard}>
              {state.id === stopAt && <TakesTheStop at={state.id} state={state.state} />}
              {(onFocus !== undefined || onBlur !== undefined) && (
                <FocusHeard onFocus={onFocus} onBlur={onBlur} />
              )}
              {twist === "lead" && toggle}
              {rowHolds(content, owned, tipped)(state)}
              {twist === "trail" && toggle}
              {state.allowsDragging && <NoDrag />}
            </RowHeard.Provider>
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
        <AriaTreeHeader {...groupHeader("tree", named)}>{title}</AriaTreeHeader>
      )}
      {children}
    </AriaTreeSection>
  );
}
