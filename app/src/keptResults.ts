// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import type { Said } from "./actions";

/** At most this many results are kept: the newest, as a log would. */
export const KEPT_RESULTS = 20;

/** **A line about something still under way**, not a result: that the core has not answered a
 *  command yet (`answer.late.*`), which goes when the answer comes, or a `progress` line on no
 *  track that the result replaces. It is never kept. A progress line on a track is kept, and
 *  its result takes its place (#3649). */
function passing(said: Said): boolean {
  return (
    (said.progress === true && said.track === undefined) || said.from.startsWith("answer.late.")
  );
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
  // A line on a track takes the place of the one kept before it there, keeping its id and its
  // place in the order: one notice that changes, never a second (#3649).
  const before =
    said.track === undefined
      ? -1
      : kept.findIndex((one) => one.plane === plane && one.said.track === said.track);
  if (before !== -1) return kept.map((one, at) => (at === before ? { ...one, said } : one));
  return [{ id: next, said, plane }, ...kept].slice(0, KEPT_RESULTS);
}

/** The results kept for `plane`'s Inbox: its own and the window's. */
export function resultsFor(kept: readonly KeptResult[], plane: string | undefined) {
  return kept.filter((one) => one.plane === undefined || one.plane === plane);
}

/** Keeps each new `said`, worked out during the render as `useLatestSaid` does. */
export function useKeptResults(said: Said | undefined, plane: string | undefined) {
  // `last` is the answer last kept: each is kept once, when it is first seen, so one put away
  // here is not kept again while it is still the newest thing said (#3344's review).
  const [kept, setKept] = useState<{
    list: readonly KeptResult[];
    next: number;
    last: Said | undefined;
  }>(() => ({ list: keep([], said, plane, 0), next: 1, last: said }));
  let list = kept.list;
  if (said !== kept.last) {
    list = keep(list, said, plane, kept.next);
    setKept({ list, next: kept.next + 1, last: said });
  }
  const drop = (id: number) =>
    setKept((was) => ({ ...was, list: was.list.filter((one) => one.id !== id) }));
  return { kept: list, drop };
}
