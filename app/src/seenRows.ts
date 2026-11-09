// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a row of the Inbox's views reads of what is New** (#6002, ADR 0121 decision 10): the
 * keys rows are seen by, and the record of the project they are drawn under (`SeenRowsOf`). Apart
 * from `seen.ts`, which talks to the core, so a page that draws a row and nothing of the
 * person's (the theme specimen) loads nothing that reads their state.
 */
import { createContext, useCallback, useContext, useEffect, useRef, type RefObject } from "react";
import type { InboxUpdate, Shown } from "./bindings";

/** How long a row is in sight in an open view before it has been seen: about a second. */
export const SEEN_AFTER_MS = 1000;

/** What every ask's key starts with: the core lets go of those no longer standing. */
export const ASK = "ask:";

/** An ask's key: its chat's, and its own, unique in its project. */
export const askSeenKey = (ask: Pick<Shown, "session" | "ask">) =>
  `${ASK}${ask.session}:${ask.ask}`;

/** A Notice's key: its cause, what its dismissal is keyed by too. */
export const noticeSeenKey = (cause: string) => `notice:${cause}`;

/** An update's key: its own, which the core keeps it by. */
export const updateSeenKey = (key: string) => `update:${key}`;

/** Whether `key` is New under `seen`: nothing is, until the record is read. */
export const isNewIn = (seen: ReadonlySet<string> | undefined, key: string) =>
  seen !== undefined && !seen.has(key);

/** The asks of `asks` that are New. */
export const newAsks = (
  asks: readonly Shown[] | undefined,
  seen: ReadonlySet<string> | undefined,
) => (asks ?? []).filter((ask) => isNewIn(seen, askSeenKey(ask))).length;

/** The updates of `updates` that are New: not seen, and not marked read. */
export const newUpdates = (
  updates: readonly InboxUpdate[] | undefined,
  seen: ReadonlySet<string> | undefined,
) => (updates ?? []).filter((one) => !one.read && isNewIn(seen, updateSeenKey(one.key))).length;

/** The Notices of `causes` that are New. */
export const newNotices = (causes: readonly string[], seen: ReadonlySet<string> | undefined) =>
  causes.filter((cause) => isNewIn(seen, noticeSeenKey(cause))).length;

/** What a row of the Inbox's views asks of the seen record: {@link useSeenRow}. */
export interface SeenRows {
  isNew: (key: string) => boolean;
  /** Watches `element` for `key`: seen once it has been in sight for {@link SEEN_AFTER_MS}.
   *  Answers the way to stop. */
  watch: (element: Element, key: string) => () => void;
}

/** The seen record of the project whose views are drawn below. None outside one: nothing New. */
export const SeenRowsOf = createContext<SeenRows | null>(null);

/**
 * **A row of an Inbox view**: whether it is New, and its element watched while it is, so it is
 * seen once it has been in sight for about a second. `drawn` is the row's own element.
 *
 * **A row the list family draws is there a commit later** (#3123): its element is not in `drawn`
 * yet when this row's effects run. So the second answer is what the row's `onDrawn` calls, after
 * each drawing of it, to watch the element once it is there.
 */
export function useSeenRow(
  key: string,
  drawn: RefObject<Element | null>,
): [isNew: boolean, drawnNow: () => void] {
  const rows = useContext(SeenRowsOf);
  const isNew = rows?.isNew(key) ?? false;
  const watching = useRef<{ element: Element; key: string; stop: () => void } | undefined>(
    undefined,
  );
  const drawnNow = useCallback(() => {
    const element = rows !== null && isNew ? drawn.current : null;
    const was = watching.current;
    if (was !== undefined && was.element === element && was.key === key) return;
    was?.stop();
    watching.current =
      element === null || rows === null
        ? undefined
        : { element, key, stop: rows.watch(element, key) };
  }, [rows, isNew, key, drawn]);
  useEffect(() => {
    drawnNow();
  }, [drawnNow]);
  useEffect(
    () => () => {
      watching.current?.stop();
      watching.current = undefined;
    },
    [],
  );
  return [isNew, drawnNow];
}
