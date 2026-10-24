// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A strip of tabs on React Aria** (#3042, spec #2897, ADR 0104): the tablist behaviour of the
 * session strip, from React Aria's tabs (`useTabListState`, `useTabList`, `useTab`), under the
 * family's parts (`Tabs.tsx`). A screen draws a tablist from these and never by hand
 * (`purlis/tabs-family`).
 *
 * **What React Aria gives the strip**, as the WAI-ARIA "Tabs" pattern has it:
 *
 * - **The keyboard along it.** Left and Right move to the tab beside, wrapping at either end;
 *   Home and End move to the first and the last; the direction follows the locale.
 * - **Manual activation.** Moving is not choosing: Enter or Space on a tab chooses it, through the
 *   strip's own press (D-3042-2).
 * - **One Tab stop.** The tab the keyboard is on is the stop while it is inside the strip; when
 *   the keyboard leaves the tabs, the stop goes back to the chosen tab ({@link useTabStrip}'s
 *   `selected`), which is where coming back in lands.
 *
 * **What stays the strip's own**, because a React Aria `TabList` cannot hold it (D-3042-1):
 *
 * - **A cell holds more than its tab**: the `×`, the task chip, the fresh mark, a browser's
 *   thumbnail. A tablist may own tabs only, so the role is on an empty element that owns the
 *   strip's tabs by id ({@link TabStripRow}, #1204), and the keyboard is heard on the element
 *   that holds the cells, for keys pressed on its tabs only.
 * - **Choosing is the catalogue's row** (`tab.select:<id>`): a press on the tab in front can do
 *   something else (go back to its chat), and a drag's release is not a press. So the tab takes
 *   React Aria's focus and roles and leaves its press to the strip.
 * - **Dragging** is `dnd-kit`'s (`sortable.tsx`): React Aria's drag and drop is for list boxes,
 *   grids, tables and trees, not tabs (D-3042-3).
 */
import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  type ComponentPropsWithoutRef,
  type FocusEvent,
  type KeyboardEvent,
  type ReactNode,
  type Ref,
} from "react";
import { mergeProps, useObjectRef, useTab, useTabList } from "react-aria";
import { Item } from "react-stately/Item";
import { useTabListState, type TabListState } from "react-stately/useTabListState";
import { Tooltip, type Tip } from "../tooltip/Tooltip";

/** What React Aria's focus scope asks an element it gives the keyboard back to (`FocusScope`). */
const RESTORING = "react-aria-focus-scope-restore";

/** The keys that move along a strip: React Aria's, and no others (not Tab, not Escape). */
const ALONG = new Set(["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"]);

/** Whether the keyboard is on one of a strip's tabs: React Aria's own collection marks. */
function keyboardOnTheStrip(): boolean {
  const at = document.activeElement;
  return (
    at instanceof HTMLElement &&
    at.dataset.collection !== undefined &&
    at.getAttribute("role") === "tab"
  );
}

/** One tab of the collection React Aria builds from the strip's keys. */
type Keyed = { key: string };

/** Each key, as an item of the collection: no words, since the strip finds no tab by typing. */
const asItem = (item: Keyed) => <Item key={item.key}>{item.key}</Item>;

/** **A strip's tabs, as React Aria holds them**: what its row and each of its tabs is handed. */
export interface TabStrip {
  state: TabListState<Keyed>;
  /** The tabs drawn, by key, in the order they are drawn. */
  keys: readonly string[];
  /** Where the keyboard comes in: the chosen tab, or the first drawn when it is not drawn. */
  home: string | undefined;
}

/**
 * **A strip's tabs, on React Aria.** `keys` are the tabs drawn, in the order they are drawn,
 * and `selected` the chosen one. Its {@link TabStripRow} and each {@link StripTab} are handed
 * what it hands back. Nothing is chosen through it: a tab is chosen by the strip's own press.
 */
export function useTabStrip({
  keys,
  selected,
}: {
  keys: readonly string[];
  selected: string | undefined;
}): TabStrip {
  // The chosen tab, or the first drawn when it is not drawn (a tab collapsed into show-more, a
  // tab being renamed).
  const home = selected !== undefined && keys.includes(selected) ? selected : keys[0];
  const joined = keys.join("\n");
  // One collection while the keys are the same, so React Aria's state keeps where it is.
  const items = useMemo(
    () => (joined === "" ? [] : joined.split("\n")).map((key) => ({ key })),
    [joined],
  );
  const state = useTabListState<Keyed>({
    items,
    children: asItem,
    selectedKey: home ?? null,
    // Choosing is the strip's (see this module); React Aria keeps where the keyboard is.
    onSelectionChange: () => {},
  });
  const manager = state.selectionManager;
  // A tab that goes (closed, collapsed, renamed) takes the stop with it: the stop comes back to
  // the chosen tab, so the strip always has one. Only the stop: the keyboard is moved only while
  // it is still on the strip (`StripTab`), and once a tab has gone it is not.
  const at = manager.focusedKey;
  const gone = at == null || !items.some((one) => one.key === String(at));
  useEffect(() => {
    if (home === undefined || !gone) return;
    if (manager.isFocused && !keyboardOnTheStrip()) manager.setFocused(false);
    manager.setFocusedKey(home);
  }, [home, gone, manager]);
  return { state, keys, home };
}

/**
 * **The row that holds a strip's cells**, where the keyboard along its tabs is heard, and its
 * tablist: an empty element first inside it, laid over it (`.strip-tablist`), which owns the
 * tabs by their element ids (`ids`, in the order they are drawn) and nothing else (#1204).
 *
 * Only the keys that move along the strip are React Aria's, and only pressed on one of its
 * tabs: a `×` or a chip beside a tab keeps its own arrows, Tab goes on to the next stop in the
 * document (the `+` among them), and a key a tab has taken (a lifted tab's arrows) is its own.
 * `className` places it, as the strip's own rules do (`.tabs`); never a look.
 */
export function TabStripRow({
  strip,
  name,
  ids,
  orientation = "horizontal",
  ref,
  children,
  ...rest
}: Omit<ComponentPropsWithoutRef<"div">, "role"> & {
  strip: TabStrip;
  /** The tablist's accessible name. */
  name: string;
  ids: readonly string[];
  orientation?: "horizontal" | "vertical";
  ref?: Ref<HTMLDivElement>;
  children?: ReactNode;
  [data: `data-${string}`]: string | number | boolean | undefined;
}) {
  const own = useObjectRef(ref);
  const { state, home } = strip;
  // **A dialog closing hands the keyboard back where it was, and the strip never takes it.**
  // React Aria's focus scope asks the element it restores to whether a collection around it
  // wants to restore focus itself (`react-aria-focus-scope-restore`), and a collection says yes
  // and puts the keyboard on its own stop. The strip's cells hold more than its tabs (the `+`,
  // a `×`), and its tabs are never reused for another, so the element itself takes it back:
  // the `+` that opened Start a chat, not the new chat's tab.
  useEffect(() => {
    const row = own.current;
    if (row === null) return;
    const handBack = (event: Event) => event.stopPropagation();
    row.addEventListener(RESTORING, handBack, true);
    return () => row.removeEventListener(RESTORING, handBack, true);
  }, [own]);
  const { tabListProps } = useTabList(
    { "aria-label": name, orientation, keyboardActivation: "manual" },
    state,
    own,
  );
  // The role and the name are the tablist's, below; the rest is the row's.
  const { onKeyDown, onKeyUp, onFocus, onBlur, onMouseDown } = tabListProps;
  const collection = (tabListProps as { "data-collection"?: string })["data-collection"];
  /** Whether `target` is one of this strip's tabs. */
  const onATab = (target: EventTarget | null) =>
    target instanceof HTMLElement &&
    target.dataset.key !== undefined &&
    target.dataset.collection === collection;
  const heard = (event: KeyboardEvent<HTMLElement>) =>
    ALONG.has(event.key) && !event.defaultPrevented && onATab(event.target);
  const manager = state.selectionManager;
  const along = {
    onKeyDown: (event: KeyboardEvent<HTMLDivElement>) => {
      if (heard(event)) onKeyDown?.(event);
    },
    onKeyUp: (event: KeyboardEvent<HTMLDivElement>) => {
      if (heard(event)) onKeyUp?.(event);
    },
    onFocus: (event: FocusEvent<HTMLDivElement>) => {
      if (onATab(event.target)) onFocus?.(event);
    },
    onBlur: (event: FocusEvent<HTMLDivElement>) => {
      onBlur?.(event);
      // Leaving the tabs, for the `+`, a `×` or anywhere else, puts the stop back on the chosen
      // tab, where the keyboard comes back in.
      if (onATab(event.relatedTarget)) return;
      manager.setFocused(false);
      if (home !== undefined) manager.setFocusedKey(home);
    },
    onMouseDown,
  };
  return (
    <div {...mergeProps(rest, along)} ref={own} data-collection={collection}>
      <span
        className="strip-tablist"
        role="tablist"
        aria-label={name}
        aria-orientation={orientation}
        aria-owns={ids.length > 0 ? ids.join(" ") : undefined}
      />
      {children}
    </div>
  );
}

/**
 * **One tab of a strip**: a `<button>` with React Aria's role, focus and Tab stop, and its
 * `tip`. Everything else (its press, its keys, a drag's handlers, its words) is the strip's,
 * handed on as they are. `selected` is whether it is the chosen tab.
 */
export function StripTab({
  strip,
  tabKey,
  selected,
  tip,
  ref,
  children,
  ...handed
}: Omit<ComponentPropsWithoutRef<"button">, "role" | "aria-selected" | "tabIndex"> & {
  strip: TabStrip;
  tabKey: string;
  selected: boolean;
  /** Its tooltip, if it has one. */
  tip: Tip;
  ref?: Ref<HTMLButtonElement>;
  children?: ReactNode;
  [data: `data-${string}`]: string | number | boolean | undefined;
}) {
  const own = useObjectRef(ref);
  const manager = strip.state.selectionManager;
  // **A tab that goes with the keyboard on it never takes the keyboard elsewhere.** No blur
  // reaches the row when an element leaves the document, so React Aria would still think the
  // strip has the keyboard and move it onto the next stop, away from wherever it went (the
  // palette, a terminal). The strip lets the keyboard go here, before the tab is removed;
  // `tabKeys.ts` brings it back to the strip where nothing else took it.
  // Once, as it leaves: the manager is a new object on each render, so it is read as it is then.
  const latest = useRef(manager);
  useLayoutEffect(() => {
    latest.current = manager;
  });
  useLayoutEffect(
    () => () => {
      const element = own.current;
      if (element !== null && element.contains(document.activeElement))
        latest.current.setFocused(false);
    },
    [own],
  );
  const { tabProps } = useTab({ key: tabKey }, strip.state, own);
  // React Aria's Tab stop, focus and marks, and nothing of its press (see this module).
  const {
    tabIndex,
    onFocus,
    "data-key": key,
    "data-collection": collection,
  } = tabProps as typeof tabProps & { "data-key"?: string; "data-collection"?: string };
  return (
    <Tooltip tip={tip}>
      <button
        {...mergeProps(handed, { onFocus })}
        ref={own}
        role="tab"
        aria-selected={selected}
        tabIndex={tabIndex}
        data-key={key}
        data-collection={collection}
      >
        {children}
      </button>
    </Tooltip>
  );
}
