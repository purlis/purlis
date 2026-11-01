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
 * - **The person can reorder the items on an end**, keeping the side an item says: a drag along
 *   the end, or from the keyboard, in the same menu, Move left and Move right, which list the items
 *   and stay up for the next move, because a cell of the bar is no Tab stop to start from. The
 *   order is the caller's to keep, as what is hidden is.
 * - **A press runs a command.** An item purlis draws is a {@link StatusItem} whose press is its
 *   command; an item given as data (an extension's) names a command, which the bar hands to
 *   `onCommand`, and one that names none is its words.
 *
 * The bar is a landmark (`footer`, named by `aria-label`) whose items are each one Tab stop. When
 * its menu closes, the keyboard is back on the item it was opened from, or, where that item was
 * just hidden, on the bar's first item the keyboard reaches.
 */
import { useRef, useState, type ReactNode } from "react";
import { DndContext, PointerSensor, closestCenter, useSensor, useSensors } from "@dnd-kit/core";
import { SortableContext, horizontalListSortingStrategy } from "@dnd-kit/sortable";
import { VisuallyHidden } from "react-aria";
import { ContextMenu } from "../menu/ContextMenu";
import { Menu, MenuItem, MenuSection, MenuSeparator, Submenu } from "../menu/Menu";
import {
  ALONG_THE_STRIP,
  DRAG_AFTER_PX,
  SortableTab,
  stripAccessibility,
  type Sortable,
} from "../../sortable";
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
 *  order given, then the person's `order` over that, hidden ones left out. */
export function placed<T extends Placed>(
  items: readonly T[],
  side: Placed["side"],
  hidden: readonly string[] = [],
  order: readonly string[] = [],
): T[] {
  const byPriority = items
    .map((item, at) => ({ item, at }))
    .filter(({ item }) => item.side === side)
    .sort((a, b) => b.item.priority - a.item.priority || a.at - b.at)
    .map(({ item }) => item);
  return inOrder(byPriority, order).filter(
    (item) => !(item.hideable !== false && hidden.includes(item.id)),
  );
}

/** **The person's order over the priorities**: what it names in its order, and an item it never
 *  named just after the one it follows by priority, so an item new to the bar moves no other. */
function inOrder<T extends Placed>(byPriority: readonly T[], order: readonly string[]): T[] {
  const at = new Map(order.map((id, place) => [id, place]));
  const drawn = byPriority
    .filter((item) => at.has(item.id))
    .sort((a, b) => (at.get(a.id) as number) - (at.get(b.id) as number));
  byPriority.forEach((item, place) => {
    if (!at.has(item.id))
      drawn.splice(place === 0 ? 0 : drawn.indexOf(byPriority[place - 1]) + 1, 0, item);
  });
  return drawn;
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

/** One item's place on the bar, and the line after it where it asks for one. */
function Slot({
  item,
  onCommand,
  sortable,
}: {
  item: StatusContribution;
  onCommand?: (command: string) => void;
  /** Absent, it stays where it is drawn. */
  sortable?: Sortable;
}) {
  const dragged = sortable?.sortable;
  return (
    <>
      <span
        ref={dragged?.setNodeRef}
        style={sortable?.style}
        data-scope="status-bar"
        data-part="slot"
        data-item={item.id}
        data-dragging={dragged?.isDragging || undefined}
        // In the capture, because a status item's own press keeps the pointer's press from what
        // is around it, and `dnd-kit` starts from that press. Only the pointer's: the keyboard's
        // way is the bar's menu, so an item is no more Tab stops than it was.
        onPointerDownCapture={(event) => dragged?.listeners?.onPointerDown?.(event)}
      >
        {drawn(item, onCommand)}
      </span>
      {item.separated === true && (
        <span data-scope="status-bar" data-part="separator" aria-hidden="true" />
      )}
    </>
  );
}

/**
 * **The status bar**: its items on two ends, and its menu, where each item the person may hide
 * is a check to hide it or show it again, and each item the bar draws can be moved along its end.
 */
export function StatusBar({
  items,
  hidden = [],
  order = [],
  onHide,
  onMove,
  onCommand,
  "aria-label": named,
  "data-testid": testid,
  className,
}: {
  items: readonly StatusContribution[];
  /** The ids the person hid. */
  hidden?: readonly string[];
  /** The order the person gave the items, by id. */
  order?: readonly string[];
  /** The person hid an item, or showed it again. Absent, the bar has no menu. */
  onHide?: (id: string, hidden: boolean) => void;
  /** The person put the item `id` where `onto` stands on its end, whose items are drawn as
   *  `shown`. Absent, the items stay where they are put. */
  onMove?: (id: string, onto: string, shown: readonly string[]) => void;
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
  // What the last move from the menu did: the keyboard stays on the menu's row, so a reader is
  // told, and a drag says itself through `dnd-kit`.
  const [moveSaid, setMoveSaid] = useState("");
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: DRAG_AFTER_PX } }),
  );
  const end = (side: Placed["side"]) => {
    const here = placed(items, side, hidden, order);
    if (onMove === undefined)
      return here.map((item) => <Slot key={item.id} item={item} onCommand={onCommand} />);
    const ids = here.map((item) => item.id);
    const nameOf = (id: unknown) => here.find((item) => item.id === id)?.name ?? String(id);
    return (
      <DndContext
        sensors={sensors}
        collisionDetection={closestCenter}
        modifiers={ALONG_THE_STRIP}
        // On the page, not in the bar: what `dnd-kit` keeps for a reader is no words of the bar's.
        accessibility={{
          announcements: stripAccessibility("item", nameOf).announcements,
          container: document.body,
        }}
        onDragEnd={({ active, over }) => {
          if (over && over.id !== active.id) onMove(String(active.id), String(over.id), ids);
        }}
      >
        <SortableContext items={ids} strategy={horizontalListSortingStrategy}>
          {here.map((item) => (
            <SortableTab key={item.id} id={item.id}>
              {(sortable) => <Slot item={item} onCommand={onCommand} sortable={sortable} />}
            </SortableTab>
          ))}
        </SortableContext>
      </DndContext>
    );
  };
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
        {onMove !== undefined && <VisuallyHidden role="status">{moveSaid}</VisuallyHidden>}
      </div>
    </footer>
  );
  if (onHide === undefined) return bar;
  const listed = placedAll(items, order).filter((item) => item.hideable !== false);
  const shown = listed.filter((item) => !hidden.includes(item.id)).map((item) => item.id);
  /** A row for every item the bar draws, in the bar's order, that moves it one place `step`. */
  const moves = (step: -1 | 1) => {
    const way = step < 0 ? "left" : "right";
    const onItsEnd = (side: Placed["side"]) => placed(items, side, hidden, order);
    return [...onItsEnd("left"), ...onItsEnd("right")].map((item) => {
      const along = onItsEnd(item.side);
      const onto = along[along.indexOf(item) + step] as StatusContribution | undefined;
      return (
        <MenuItem
          key={item.id}
          id={item.id}
          textValue={item.name}
          // Unavailable and not disabled: a row taken from under the keyboard as an item reaches
          // its end's edge would lose it, and this menu stays up for the next move.
          aria-disabled={onto === undefined}
          tooltip={
            onto === undefined
              ? `Already ${step < 0 ? "first" : "last"} on its end of the bar`
              : undefined
          }
          shouldCloseOnSelect={false}
          onAction={() => {
            if (onto === undefined) return;
            onMove?.(
              item.id,
              onto.id,
              along.map((one) => one.id),
            );
            setMoveSaid(`${item.name} moved ${way} of ${onto.name}`);
          }}
          data-testid={`status-move-${way}-${item.id}`}
        >
          {item.name}
        </MenuItem>
      );
    });
  };
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
          {onMove !== undefined && <MenuSeparator />}
          {onMove !== undefined && (
            <MenuSection>
              <Submenu
                aria-label="Move left"
                row={
                  <MenuItem id="move-left" textValue="Move left">
                    Move left
                  </MenuItem>
                }
              >
                {moves(-1)}
              </Submenu>
              <Submenu
                aria-label="Move right"
                row={
                  <MenuItem id="move-right" textValue="Move right">
                    Move right
                  </MenuItem>
                }
              >
                {moves(1)}
              </Submenu>
            </MenuSection>
          )}
        </Menu>
      )}
    >
      {bar}
    </ContextMenu>
  );
}

/** Every item, the left end's then the right end's, each in its order: the menu's rows. */
function placedAll(
  items: readonly StatusContribution[],
  order: readonly string[],
): StatusContribution[] {
  return [...placed(items, "left", [], order), ...placed(items, "right", [], order)];
}
