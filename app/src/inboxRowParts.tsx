// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useLayoutEffect, useRef, useState } from "react";
import { SETTLE_MS } from "./TaskBlocksNotice";
import { useWayPlaces } from "./wayPlaces";

// **The pieces every Inbox view's rows share** (moved from `Inbox.tsx`): how they say a chain and a
// time, and the settling an Allow waits for.

/** What a chain is drawn as: the session first, the chat that asked last (I-9). */
export const chainSaid = (chain: readonly string[]) => chain.join(" › ");

/** The time an answer was given, or an update happened, as the list says it. */
export const timeSaid = (at: number) =>
  new Date(at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

/** The time now: when an update was drawn where it stands, or when an answer was pressed. */
export const placedAt = () => performance.now();

/** A row's answers that grant: an ask's or an update's Allow, or a way out of its Notice's. */
export const granting = (row: Element) => row.querySelectorAll('[data-variant="grants"], .allows');
export const standsFirst = () => false;

/**
 * **When a row was last drawn where it stands, or moved** (#1693, #1695): what an Allow on it
 * waits {@link SETTLE_MS} after, so what is under the pointer is what the person read.
 *
 * `here` is what the row stands under and offers: what is drawn above it and its own words and
 * answers. It is taken in the very render that draws it, so no press lands on a place whose time
 * is not known yet. **Where each of its answers that grant was last drawn, and since when**
 * (#3167, `useWayPlaces`): something above it can grow without a stop of its own changing (a
 * sentence said under another row, the recent answers of an empty list), its own words can wrap
 * again at a new width, and the list can be made to scroll back; none of these need reach this
 * row's render. So each answer's own box is read after each render, on every change that can
 * move it, and again at the press: one that differs is a move.
 */
export function useSettledPlace<T extends HTMLElement>(
  here: string,
  /** **Heard as the row is attached** (#6002): a row a list draws anew (a scope picked, #6340)
   *  is attached after its drawing is said, and nothing may draw it again, so what has to read
   *  the row, such as its watch to be seen, reads it here too. */
  attached?: () => void,
) {
  const [placed, setPlaced] = useState(() => ({ here, at: placedAt() }));
  if (placed.here !== here) setPlaced({ here, at: placedAt() });
  const drawn = useRef<T>(null);
  // Its first drawing is `placed`'s: an answer read for the first time is no move of its own.
  const { movedAt } = useWayPlaces(drawn, granting, standsFirst);
  const tooSoon = () => placedAt() - Math.max(placed.at, movedAt()) < SETTLE_MS;
  /** Reads where its answers stand now: after each drawing of its row, which the list family
   *  draws a commit after this hook's own render (`ListRowDrawn`'s `onDrawn`, #3123). */
  const readPlaces = () => {
    movedAt();
  };
  /**
   * **The row as a callback ref, its answers read where they stand once it is on the page**
   * (#5998): the list family attaches a row a commit after its drawing is said (`onDrawn`), so an
   * answer drawn on the row from the first (a decision's Allow) is read there, and never first
   * at the next frame.
   */
  // One function for the row's life: a new one each render would detach the row and attach it
  // again on every commit, and whatever reads `drawn` in between would find no row (#6002).
  const lastMovedAt = useRef(movedAt);
  const lastAttached = useRef(attached);
  useLayoutEffect(() => {
    lastMovedAt.current = movedAt;
    lastAttached.current = attached;
  });
  const drawnAt = useCallback((row: T | null) => {
    drawn.current = row;
    if (row === null) return;
    lastMovedAt.current();
    lastAttached.current?.();
  }, []);
  return { drawn, drawnAt, tooSoon, readPlaces };
}

/**
 * **What a press that did nothing says, where it was pressed** (#1693, #1695): an answer on an
 * update, or a way out of a granting Notice, pressed on something drawn or moved just now.
 */
export function NothingDone({ said }: { said: string | undefined }) {
  return said === undefined ? null : (
    <p className="inbox-refused" role="status">
      {said}
    </p>
  );
}
