// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A row's anatomy** (#2911, spec #2897, ADR 0103): what a list's, a tree's and a table's row
 * holds, in the same places in all three. A row is the family's root; inside it its `lead` (a
 * mark, an icon, a persona's tile), its `label` (the words, which take the room and give way to
 * an ellipsis), the one `description` that may stand beside or under them, its `trail` (a
 * count, a state, the row's actions) at its end, and its `shortcut`, the key that does the same
 * (#2921), last.
 *
 * Each slot carries the `data-scope` of the family whose row it is in (`RowScope`), so a theme's
 * `list.label.text` reaches a list's words and never a tree's.
 */
import { createContext, useContext, type CSSProperties, type ReactNode } from "react";
import { Text as AriaText } from "react-aria-components";

/** The families a row belongs to. */
export type RowFamily = "list" | "tree" | "table";

/** The family the row a slot is drawn in belongs to. */
export const RowScope = createContext<RowFamily>("list");

/**
 * **Inside an option** (`ListOption`): its words are its name and its one line about it its
 * description, to a reader too (React Aria's `label` and `description` slots), rather than the
 * whole row's content run together.
 */
export const InOption = createContext(false);

/** The sizes a row is drawn at: 24, 28 and 32 px at the default text size and density. */
export const ROW_SIZES = ["sm", "md", "lg"] as const;
export type RowSize = (typeof ROW_SIZES)[number];

/** What every slot takes: what it holds, and a place in its row (never a look). */
interface Slot {
  children?: ReactNode;
  /** A place for it in its row: margin, width, order. Never a look. */
  className?: string;
  style?: CSSProperties;
  id?: string;
  /** `data-*` attributes reach the element as they are. */
  [data: `data-${string}`]: string | number | boolean | undefined;
}

function slot(part: string, named?: "label" | "description") {
  function RowSlot({ children, ...rest }: Slot) {
    const scope = useContext(RowScope);
    const option = useContext(InOption);
    if (option && named !== undefined)
      return (
        <AriaText {...rest} slot={named} elementType="span" data-scope={scope} data-part={part}>
          {children}
        </AriaText>
      );
    return (
      <span {...rest} data-scope={scope} data-part={part}>
        {children}
      </span>
    );
  }
  RowSlot.displayName = `Row.${part}`;
  return RowSlot;
}

/** **Its marks**, before its words: a state, an icon, a persona's tile. */
export const RowLead = slot("lead");

/** **Its words**: one line that takes the room and gives way to an ellipsis, never wraps. */
export const RowLabel = slot("label", "label");

/**
 * **The one line about it**, beside its words or under them, in the quiet tier: one to a row.
 * In an option it is the option's description, which a reader hears after its name.
 */
export const RowDescription = slot("description", "description");

/** **Its end**: a count, a state, the row's own actions. */
export const RowTrail = slot("trail");

/**
 * **A line under the row's words, inside the row**: what a search matched in its body, why it
 * was refused. The row is as tall as its two lines; it is still one row to the keyboard.
 */
export const RowBelow = slot("below");

/**
 * **The key that does the same**, at the row's end and read last (#2921, N25): a palette row's,
 * from the key registry (`keys/registry.ts`). Drawn, and hidden from a reader, who hears the key
 * by its name from the row's `aria-keyshortcuts` instead of as symbols run onto its words. Where
 * the key is what the row is about (the cheat sheet's), it is `heard`: its symbols are drawn and
 * its `spoken` words ("Command Shift Return") are what a reader hears.
 */
export function RowShortcut({
  keys,
  heard = false,
  spoken,
  className,
}: {
  keys: string;
  heard?: boolean;
  /** The key in words, heard in place of the symbols drawn. */
  spoken?: string;
  className?: string;
}) {
  const scope = useContext(RowScope);
  return (
    <kbd
      className={className}
      data-scope={scope}
      data-part="shortcut"
      aria-hidden={heard ? undefined : "true"}
    >
      {heard && spoken !== undefined ? (
        <>
          <span aria-hidden="true">{keys}</span>
          <span data-heard-as="">{spoken}</span>
        </>
      ) : (
        keys
      )}
    </kbd>
  );
}
