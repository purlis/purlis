// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The status bar and its contribution model** (N28, the operator's #2814·5, spec #2897): one
 * model for every item on the bar, purlis's own and an extension's alike.
 *
 * - **An item sits left or right, by priority.** Each side draws its items with the highest
 *   priority first, from its left; items of one priority keep the order they were given in. The
 *   left end is where the person is, and gives way first in a narrow window, its last item
 *   first; the right end is what wants a look, and never gives way.
 * - **The person can hide any item**, and show it again, from the bar's own menu: a right click
 *   on the bar, or the menu key (or Shift F10) on any item in it. What is hidden is the caller's
 *   to keep (`statusBarPrefs.ts`), by the item's id, so an item that is gone today stays hidden
 *   when it comes back.
 * - **A press runs a command.** An item purlis draws is a {@link StatusItem} whose press is its
 *   command; an item given as data (an extension's) names a command, which the bar hands to
 *   `onCommand`, and one that names none is its words.
 *
 * The bar is a landmark (`footer`, named by `aria-label`) whose items are each one Tab stop. When
 * its menu closes, the keyboard is back on the item it was opened from, or, where that item was
 * just hidden, on the bar's first item the keyboard reaches.
 */
import { Fragment, useRef, type ReactNode } from "react";
import { ContextMenu } from "../menu/ContextMenu";
import { Menu, MenuItem, MenuSection } from "../menu/Menu";
import { StatusCell, StatusItem } from "./StatusItem";

/** What every item of the bar says about itself. */
type Placed = {
  /** Stable across launches: what the person's hiding is kept by. */
  id: string;
  /** What the bar's menu calls it. */
  name: string;
  side: "left" | "right";
  /** Higher first, from the side's left. */
  priority: number;
  /** Whether the person may hide it. Every item may unless it says otherwise; a bar keeps one
   *  item it cannot lose that the keyboard reaches, so its menu always opens from one. */
  hideable?: boolean;
  /** A line after it, parting it from what follows: the separator part. */
  separated?: boolean;
};

/** **An item purlis draws itself**, already a status item: its press is its command. */
export type DrawnContribution = Placed & { item: ReactNode };

/** **An item given as data**: an extension's, drawn by the bar. */
export type DataContribution = Placed & {
  /** Its words, and its value after them. */
  label: string;
  value?: string;
  /** Its tooltip: where it came from, and how old it is. */
  tooltip?: string;
  /** Older than it declared fresh: drawn quiet. */
  stale?: boolean;
  /** The command a press runs, by its id, handed to the bar's `onCommand`. */
  command?: string;
  testid?: string;
};

export type StatusContribution = DrawnContribution | DataContribution;

/** **The items a side draws, in its order**: that side's, highest priority first, ties in the
 *  order given, hidden ones left out. */
export function placed<T extends Placed>(
  items: readonly T[],
  side: Placed["side"],
  hidden: readonly string[] = [],
): T[] {
  return items
    .map((item, at) => ({ item, at }))
    .filter(
      ({ item }) => item.side === side && !(item.hideable !== false && hidden.includes(item.id)),
    )
    .sort((a, b) => b.item.priority - a.item.priority || a.at - b.at)
    .map(({ item }) => item);
}

function drawn(item: StatusContribution, onCommand?: (command: string) => void): ReactNode {
  if ("item" in item) return item.item;
  const { label, value, tooltip, stale, command, testid } = item;
  if (command !== undefined && onCommand !== undefined)
    return (
      <StatusItem
        data-testid={testid}
        tone={stale ? "quiet" : undefined}
        tooltip={tooltip}
        aria-label={value === undefined ? label : `${label}: ${value}`}
        count={value}
        onPress={() => onCommand(command)}
      >
        {label}
      </StatusItem>
    );
  return (
    <StatusCell
      data-testid={testid}
      tone={stale ? "quiet" : undefined}
      tooltip={tooltip}
      count={value}
    >
      {label}
    </StatusCell>
  );
}

/**
 * **The status bar**: its items on two ends, and its menu, where each item the person may hide
 * is a check to hide it or show it again.
 */
export function StatusBar({
  items,
  hidden = [],
  onHide,
  onCommand,
  "aria-label": named,
  "data-testid": testid,
  className,
}: {
  items: readonly StatusContribution[];
  /** The ids the person hid. */
  hidden?: readonly string[];
  /** The person hid an item, or showed it again. Absent, the bar has no menu. */
  onHide?: (id: string, hidden: boolean) => void;
  /** Runs an item's command, for an item given as data. */
  onCommand?: (command: string) => void;
  "aria-label": string;
  "data-testid"?: string;
  /** A place for the bar, never a look. */
  className?: string;
}) {
  // The item the menu was opened from, which the keyboard goes back to when it closes.
  const opener = useRef<HTMLElement | null>(null);
  const footer = useRef<HTMLElement | null>(null);
  const end = (side: Placed["side"]) =>
    placed(items, side, hidden).map((item) => (
      <Fragment key={item.id}>
        <span data-scope="status-bar" data-part="slot" data-item={item.id}>
          {drawn(item, onCommand)}
        </span>
        {item.separated === true && (
          <span data-scope="status-bar" data-part="separator" aria-hidden="true" />
        )}
      </Fragment>
    ));
  const bar = (
    <footer
      ref={footer}
      onContextMenuCapture={(event) => {
        const from = (event.target as Element).closest<HTMLElement>("button, [tabindex]");
        opener.current = from ?? (document.activeElement as HTMLElement | null);
      }}
      data-scope="status-bar"
      aria-label={named}
      data-testid={testid}
      className={className}
    >
      <div data-scope="status-bar" data-part="start">
        {end("left")}
      </div>
      <div data-scope="status-bar" data-part="end">
        {end("right")}
      </div>
    </footer>
  );
  if (onHide === undefined) return bar;
  const listed = [...placedAll(items)].filter((item) => item.hideable !== false);
  const shown = listed.filter((item) => !hidden.includes(item.id)).map((item) => item.id);
  return (
    // The menu family's context menu (#2909): a right click on the bar, or the menu key or
    // Shift F10 on any item in it, opens it at that item.
    <ContextMenu
      opensFromWithin
      onClosed={() => {
        // Back on the item the menu was opened from; one just hidden is gone, so the bar's
        // first item the keyboard reaches takes it.
        const back = opener.current?.isConnected
          ? opener.current
          : footer.current?.querySelector<HTMLElement>(
              'button:not([disabled]), [tabindex]:not([tabindex="-1"])',
            );
        back?.focus();
      }}
      menu={() => (
        <Menu aria-label={`${named}: the items shown`}>
          <MenuSection
            title="Show on the bar"
            selectionMode="multiple"
            selectedKeys={shown}
            // A row's press is a check turned, and closes the menu as a row's does.
            shouldCloseOnSelect
          >
            {listed.map((item) => (
              <MenuItem
                key={item.id}
                id={item.id}
                textValue={item.name}
                onAction={() => onHide(item.id, !hidden.includes(item.id))}
                data-testid={`status-show-${item.id}`}
              >
                {item.name}
              </MenuItem>
            ))}
          </MenuSection>
        </Menu>
      )}
    >
      {bar}
    </ContextMenu>
  );
}

/** Every item, the left end's then the right end's, each in its order: the menu's rows. */
function placedAll(items: readonly StatusContribution[]): StatusContribution[] {
  return [...placed(items, "left"), ...placed(items, "right")];
}
