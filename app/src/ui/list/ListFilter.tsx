// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useMemo, useState, type KeyboardEventHandler } from "react";
import { Search } from "lucide-react";
import { TextField } from "../field/Field";

/**
 * **A list over about seven items gets a filter** (#2911, R164; the map's Dialogs rule, #2910):
 * from {@link FILTERED_FROM} items the list is drawn under a filter field, the list's main one, at
 * the large size (32 px), with the total in its placeholder ("Filter 28 personas", R11). The list
 * family's (`ui/list`): a dialog's list, a panel's and a settings table's take the same one.
 *
 * **A filter keeps what was typed and where**: its value is the caller's state, so drawing the
 * list again keeps the field, its focus and its caret (R168).
 */

/** The fewest items a list has for it to be filtered: more than about seven. */
export const FILTERED_FROM = 8;

/** What a list's items are called, one and many: `chat`, `chats`. */
export type Noun = { one: string; many: string };

/**
 * **A list's filter**: what is typed, and the items it leaves, matched by a case-insensitive
 * substring of each item's name (R12). `filtered` is false for a list too short to need one,
 * which shows every item; a quick picker's list, where typing is how it is used, is filtered
 * however short (`always`).
 */
export function useListFilter<T>(
  items: readonly T[],
  nameOf: (item: T) => string,
  { always = false }: { always?: boolean } = {},
) {
  const [query, setQuery] = useState("");
  const filtered = always || items.length >= FILTERED_FROM;
  const shown = useMemo(() => {
    const typed = query.trim().toLowerCase();
    if (!filtered || typed === "") return items;
    return items.filter((item) => nameOf(item).toLowerCase().includes(typed));
  }, [items, nameOf, query, filtered]);
  return { query, setQuery, shown, filtered };
}

/**
 * **A quick picker's filter keys, as a palette's** (S8): Enter takes the first match, `first()`,
 * and the down arrow goes into the list `listId`, onto its first row that can be chosen, where
 * the arrows walk it and Enter takes the one on.
 */
export function quickPickKeys<T>(
  listId: string,
  first: () => T | undefined,
  pick: (item: T) => void,
): KeyboardEventHandler<HTMLInputElement> {
  return (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      const item = first();
      if (item !== undefined) pick(item);
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      document
        .getElementById(listId)
        ?.querySelector<HTMLElement>('[role="option"]:not([aria-disabled="true"])')
        ?.focus();
    }
  };
}

/** The line a filter that matches nothing draws in place of the list (R15). */
export function noMatchSaid(noun: Noun, query: string): string {
  return `No ${noun.one} is called “${query.trim()}”.`;
}

/**
 * **The filter field**, drawn over the list it narrows. Labelled by what it filters; ↑ and ↓ in
 * it are the caller's (`onKeyDown`), to move through the list from the field.
 */
export function ListFilter({
  noun,
  total,
  query,
  onQuery,
  onKeyDown,
  controls,
}: {
  noun: Noun;
  total: number;
  query: string;
  onQuery: (query: string) => void;
  onKeyDown?: KeyboardEventHandler<HTMLInputElement>;
  /** The id of the list it narrows. */
  controls?: string;
}) {
  return (
    // The field family's box (#2905) at the large size, a list's main filter field, with its
    // glass; it keeps the keyboard and its caret as the list it narrows is drawn again (R168).
    <TextField
      className="list-filter"
      type="search"
      size="lg"
      icon={<Search aria-hidden="true" />}
      keepsFocus={`list-filter:${noun.many}`}
      aria-label={`Filter ${noun.many}`}
      aria-controls={controls}
      placeholder={`Filter ${total} ${total === 1 ? noun.one : noun.many}`}
      value={query}
      onChange={onQuery}
      onKeyDown={onKeyDown}
    />
  );
}
