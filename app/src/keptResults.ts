// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from "react";
import type { Said } from "./actions";
import { followLandings } from "./repoClones";

/** At most this many results are kept: the newest, as a log would. */
export const KEPT_RESULTS = 20;

/** **A line about something still under way**, not a result: that the core has not answered a
 *  command yet (`answer.late.*`), which goes when the answer comes, or a `progress` line on no
 *  track that the result replaces. It is never kept. A progress line on a track is kept, and
 *  its result takes its place (#3649). */
export function passing(said: Said): boolean {
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
  keptBefore: readonly KeptResult[],
  said: Said | undefined,
  plane: string | undefined,
  first: number,
  /** The tracks whose line was put away (`trackOf`): their counting is kept no more, though
   *  the result that ends one is (#3649's review). */
  dropped: ReadonlySet<string> = NONE,
): readonly KeptResult[] {
  let kept = keptBefore;
  let next = first;
  if (said === undefined || passing(said) || kept.some((one) => one.said === said)) return kept;
  // The line said just before it, which no render may have drawn, is kept first (#3649), even
  // where the line that brings it is on a track put away (#3898): the two are separate notices.
  const after = said.after;
  if (after !== undefined && !passing(after) && !kept.some((one) => one.said === after)) {
    kept = [{ id: next, said: after, plane }, ...kept];
    next += 1;
  }
  if (said.progress === true && said.track !== undefined && dropped.has(trackOf(said, plane)))
    return kept.slice(0, KEPT_RESULTS);
  // A line on a track takes the place of the one kept before it there, keeping its id and its
  // place in the order: one notice that changes, never a second (#3649).
  const before =
    said.track === undefined
      ? -1
      : kept.findIndex((one) => one.plane === plane && one.said.track === said.track);
  if (before !== -1)
    return kept.map((one, at) => (at === before ? { ...one, said } : one)).slice(0, KEPT_RESULTS);
  return [{ id: next, said, plane }, ...kept].slice(0, KEPT_RESULTS);
}

const NONE: ReadonlySet<string> = new Set();

/** **A line's track in its project**, as the put-away tracks are named. */
export const trackOf = (said: Said, plane: string | undefined) =>
  `${plane ?? ""}\u0000${said.track ?? ""}`;

/** The id after every one `list` holds and `next`: a line can bring the one before it. */
const nextId = (list: readonly KeptResult[], next: number) =>
  Math.max(next - 1, ...list.map((one) => one.id)) + 1;

/**
 * **`repo` landed in `workspace`, so a line saying it failed there is no longer true** (#4394). A
 * line that named only it goes; one that named others stays, in its place and on its track, and
 * says only those. Answers the lines as they now are and the ones that stopped being what was said.
 */
export function landed(
  kept: readonly KeptResult[],
  plane: string | undefined,
  workspace: string,
  repo: string,
): { list: readonly KeptResult[]; overtaken: readonly Said[] } {
  const overtaken: Said[] = [];
  const list = kept.flatMap((one) => {
    const about = one.said.notCloned;
    if (one.plane !== plane || about?.workspace !== workspace) return [one];
    const rest = about.failed.filter((failed) => failed.repo !== repo);
    if (rest.length === about.failed.length) return [one];
    overtaken.push(one.said);
    return rest.length === 0 ? [] : [{ ...one, said: { ...one.said, ...about.told(rest) } }];
  });
  return { list, overtaken };
}

/** The results kept for `plane`'s Inbox: its own and the window's. */
export function resultsFor(kept: readonly KeptResult[], plane: string | undefined) {
  return kept.filter((one) => one.plane === undefined || one.plane === plane);
}

/** Keeps each new `said`, worked out during the render as `useLatestSaid` does. */
export function useKeptResults(said: Said | undefined, plane: string | undefined) {
  // `last` is the answer last kept: each is kept once, when it is first seen, so one put away
  // here is not kept again while it is still the newest thing said (#3344's review).
  // `dropped` names each track whose line was put away: it counts on unseen, and only the result
  // that ends it is kept again (#3649's review). `overtaken` holds the lines a landing changed: a
  // project's answer stays its answer, and comes back with its project, but is not true any more.
  const [kept, setKept] = useState<{
    list: readonly KeptResult[];
    next: number;
    last: Said | undefined;
    dropped: ReadonlySet<string>;
    overtaken: ReadonlySet<Said>;
  }>(() => {
    const list = keep([], said, plane, 0);
    return { list, next: nextId(list, 0), last: said, dropped: NONE, overtaken: new Set() };
  });
  useEffect(
    () =>
      followLandings((into, workspace, repo) =>
        setKept((was) => {
          const { list: now, overtaken } = landed(was.list, into, workspace, repo);
          if (overtaken.length === 0) return was;
          return { ...was, list: now, overtaken: new Set([...was.overtaken, ...overtaken]) };
        }),
      ),
    [],
  );
  let list = kept.list;
  if (said !== kept.last) {
    if (said === undefined || !kept.overtaken.has(said))
      list = keep(list, said, plane, kept.next, kept.dropped);
    // A result ends its track's putting away: a Retry after it counts again.
    const ends = said !== undefined && said.track !== undefined && said.progress !== true;
    const dropped = ends
      ? new Set([...kept.dropped].filter((one) => one !== trackOf(said, plane)))
      : kept.dropped;
    // A line can bring the one said before it (`Said.after`): the next id is past both.
    setKept({ ...kept, list, next: nextId(list, kept.next), last: said, dropped });
  }
  // One function for every render, so what is handed it is not drawn again for it (#3653).
  const drop = useCallback(
    (id: number) =>
      setKept((was) => {
        const gone = was.list.find((one) => one.id === id);
        return {
          ...was,
          list: was.list.filter((one) => one.id !== id),
          dropped:
            gone?.said.track === undefined
              ? was.dropped
              : new Set([...was.dropped, trackOf(gone.said, gone.plane)]),
        };
      }),
    [],
  );
  return { kept: list, drop };
}
