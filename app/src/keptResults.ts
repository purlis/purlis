// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import type { Said } from "./actions";

/** At most this many results are kept: the newest, as a log would. */
export const KEPT_RESULTS = 20;

/** **A line about something still under way**, not a result: that the core has not answered a
 *  command yet (`answer.late.*`), which goes when the answer comes. It is never kept. */
function passing(said: Said): boolean {
  return said.from.startsWith("answer.late.");
}

/** A result the window keeps (#3344): what was said, where, and its place in the order. */
export type KeptResult = { id: number; said: Said; plane: string | undefined };

/**
 * **What the window keeps of its results** (#3344): each answer the window's line said, once,
 * newest first, with the project that was in front, so the Inbox's Notices list it under
 * Results after its notice at the top of the content is closed. Closing that notice keeps the
 * result; putting one away here is `drop`.
 */
export function keep(
  kept: readonly KeptResult[],
  said: Said | undefined,
  plane: string | undefined,
  next: number,
): readonly KeptResult[] {
  if (said === undefined || passing(said) || kept.some((one) => one.said === said)) return kept;
  return [{ id: next, said, plane }, ...kept].slice(0, KEPT_RESULTS);
}

/** The results kept for `plane`'s Inbox: its own and the window's. */
export function resultsFor(kept: readonly KeptResult[], plane: string | undefined) {
  return kept.filter((one) => one.plane === undefined || one.plane === plane);
}

/** Keeps each new `said`, worked out during the render as `useLatestSaid` does. */
export function useKeptResults(said: Said | undefined, plane: string | undefined) {
  const [kept, setKept] = useState<{ list: readonly KeptResult[]; next: number }>(() => ({
    list: keep([], said, plane, 0),
    next: 1,
  }));
  let list = kept.list;
  if (said !== undefined && !passing(said) && !list.some((one) => one.said === said)) {
    list = keep(list, said, plane, kept.next);
    setKept({ list, next: kept.next + 1 });
  }
  const drop = (id: number) =>
    setKept((was) => ({ ...was, list: was.list.filter((one) => one.id !== id) }));
  return { kept: list, drop };
}
