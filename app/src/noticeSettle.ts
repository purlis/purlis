// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { createContext, useCallback, useLayoutEffect, useRef, useState } from "react";
import { useWayPlaces, type WayPlace } from "./wayPlaces";

/**
 * How long after a question formed, or after the tasks it lists changed, a press on it does
 * nothing: a guard against a press aimed at what was there before. It only blunts a misclick;
 * what binds an answer to what was shown is the core's check of each task's block (D-1508-9).
 *
 * **Every hold is measured on `performance.now()`** (#3227), a clock that only goes forward: a
 * step of the wall clock (a time sync, the person setting it) neither keeps a Notice held until
 * the clock catches up nor lets a press through early.
 */
export const SETTLE_MS = 1500;

/** What a press on a granting Notice drawn or moved just now says: nothing was done. */
export const NOTICE_MOVED_JUST_NOW =
  "That Notice was drawn or moved just now, so nothing was done. Read it and press again.";

/**
 * **Whether a Notice grants something** (#1695): its ways out allow a host for a persona or a
 * chat, accept a teammate's dispatch grant, allow a dispatch, answer a task's permission
 * prompt, or remove a workspace a chat asked to have removed (qw356). Its presses wait for it
 * to settle where it is drawn.
 */
export const grants = (cause: string) =>
  (cause.startsWith("sandbox-blocked:") && !cause.endsWith(":allowed")) ||
  cause.startsWith("sandbox-hosts:persona:") ||
  cause.startsWith("task-permission:") ||
  cause.startsWith("workspace-removal:") ||
  cause === "dispatch-grants" ||
  cause.startsWith("dispatch-grant:");

/** **Whether a way out grants by its look**: drawn as the button family's grants variant, as
 *  every Notice answer with `allows` is (ADR 0097 §6). */
export const grantsByLook = (way: Element) => way.closest('[data-variant="grants"]') !== null;

/**
 * **What a press on a Notice's way out asks first** (#1695, #2904): whether the place it is
 * drawn in lets it act now. A list of Notices that come and go provides one ({@link
 * useNoticeSettle}'s `allows`); a Notice standing alone acts at once. It is asked in the press
 * itself, so a press from the keyboard waits as one from the pointer does.
 */
export const NoticeWayGuard = createContext<(way: Element) => boolean>(() => true);

/** **Whether a way out of the Notice whose cause is `cause` waits to settle**: it grants by its
 *  look, or its Notice grants by its cause. */
const waits = (way: Element, cause: string) => grantsByLook(way) || grants(cause);

/** **What a pane's band remembers past its own drawing** (#3167, D-3167-6): whether its last read
 *  found a Notice, when it last went empty, and where each of its answers that grant was. */
type BandMemory = { hadAny: boolean; emptiedAt: number; places: Map<string, WayPlace> };

/**
 * **Each pane's band memory, by the pane** ({@link paneBand}): a band drawn again by another
 * component (its pane's task ended or moved, a split formed or went, the tab was switched away
 * and back) reads on from where the pane's band was, so what stood there still counts. A pane
 * drawn for the first time has none. {@link forgetPaneBand} lets a pane closed for good go.
 */
const bands = new Map<string, BandMemory>();

/** The band memory's name for pane `pane` of plane `plane`: a pane's number is never reused. */
export const paneBand = (plane: string, pane: number) => `${plane}\u0000${pane}`;

/** Lets the band memory of a pane closed for good go. */
export function forgetPaneBand(band: string) {
  bands.delete(band);
}

/** Lets every pane's band memory of plane `plane` go, as the window lets the project go; with
 *  no plane, every pane's of the window (a test's window is a new one). */
export function forgetPaneBands(plane?: string) {
  for (const band of [...bands.keys()]) {
    if (plane === undefined || band.startsWith(`${plane}\u0000`)) bands.delete(band);
  }
}

const freshBand = (): BandMemory => ({ hadAny: false, emptiedAt: -Infinity, places: new Map() });

/**
 * **Carries pane `from`'s band memory to pane `to`** (#3271): a chat given a new pane (a tab of
 * its own, a pane beside its session) has its Notices drawn there now, so what stood in the band
 * they were drawn in still counts. Carried as a copy, since the pane it left goes on: whether that
 * band held a Notice, when it went empty, and where each of its grants was. Into a band that has
 * read already it only adds, so it can only hold more.
 */
export function carryPaneBand(from: string, to: string) {
  const was = bands.get(from);
  if (was === undefined) return;
  const into = bands.get(to);
  if (into === undefined) {
    bands.set(to, { hadAny: was.hadAny, emptiedAt: was.emptiedAt, places: new Map(was.places) });
    return;
  }
  into.hadAny ||= was.hadAny;
  into.emptiedAt = Math.max(into.emptiedAt, was.emptiedAt);
  for (const [name, place] of was.places) if (!into.places.has(name)) into.places.set(name, place);
}

/**
 * **The Notices' move-settle guard** (#1695, train 50's and train 54's reviews): a Notice that
 * grants something is drawn among others that come and go, so a press on one of its ways out
 * within {@link SETTLE_MS} of that way out moving does nothing, and says why.
 *
 * **What is read is each granting way out's own box on screen** (#3167, `useWayPlaces`), never
 * its line's: the line can keep its top while a way out in it moves (its words wrap again at a
 * new width), and the band that scrolls the line can be made to scroll back (what is below it
 * got shorter) and slide it on screen while its place inside the band stays put. Both are moves;
 * the person's own scroll is not.
 *
 * `firstDrawn` says whether a Notice's first drawing counts as a move. In the Inbox it does, as
 * an ask's Allow waits on its first drawing. On a pane it does not when nothing else is there to
 * be aimed at: a pane's first Notice arrives where the person is already looking. Anything drawn
 * where something else may have been aimed at is a move: a granting way out drawn into a Notice
 * already standing, or a granting Notice drawn while the band holds another (however long that
 * one stood) or went empty within the settle time. `band` names the pane ({@link paneBand}), so
 * this holds across the band being drawn again: on the redrawing's own first read, a grant whose
 * place is remembered under the same name (its Notice's cause and words, its label and order) is
 * read against that place, and any other grant arriving in a pane whose band held a Notice is a
 * move. A grant that arrives in any later read is new, however it is named. A band with no name
 * remembers only while it is drawn, and in the Inbox no grant is ever taken for one before it.
 *
 * `drawn` says whether the root is drawn now (#3302): a pane's band is not while it holds no
 * Notice, and a read then would see every answer at no size. Nothing is read until it is, so
 * the first read, and the redrawing's, see the boxes the person sees.
 */
export function useNoticeSettle<T extends HTMLElement>(
  firstDrawn: "moves" | "stands",
  band?: string,
  drawn?: (root: T) => boolean,
) {
  const at = useRef<T>(null);
  const [said, setSaid] = useState<string>();
  // The band's memory: the pane's, read on from wherever it was drawn before, or this
  // drawing's own.
  const own = useRef<BandMemory>(undefined);
  const named = useRef(band);
  useLayoutEffect(() => {
    named.current = band;
  });
  const memory = useCallback((): BandMemory => {
    const name = named.current;
    if (name === undefined) return (own.current ??= freshBand());
    let kept = bands.get(name);
    if (kept === undefined) bands.set(name, (kept = freshBand()));
    return kept;
  }, []);
  // The lines read so far; for each way out whether its first drawing is a move, and the name
  // its place is remembered by.
  const lines = useRef(new WeakSet<Element>());
  const later = useRef(new WeakMap<Element, boolean>());
  const names = useRef(new WeakMap<Element, string>());
  // **Only this drawing's first read is a redrawing** (D-3167-10): a grant found then may be
  // the one the pane's band drew before; one that arrives in any later read is new, whatever
  // it is named. In the Inbox ("moves") a first drawing is always a move, so none is recalled.
  const firstRead = useRef(true);
  const recalling = useRef(false);
  const ways = useCallback(
    function* (root: T) {
      recalling.current = firstRead.current && firstDrawn === "stands";
      firstRead.current = false;
      const kept = memory();
      const all = [...root.querySelectorAll("[data-cause]")];
      const now = performance.now();
      // **Drawn where another Notice stood** (#3167): the last read found one here, however
      // long it had stood, or the band went empty within the settle time, so a pointer may be
      // aimed at what this one now covers.
      const held = kept.hadAny || now - kept.emptiedAt < SETTLE_MS;
      if (all.length === 0 && kept.hadAny) kept.emptiedAt = now;
      kept.hadAny = all.length > 0;
      for (const line of all) {
        const known = lines.current.has(line);
        lines.current.add(line);
        const cause = line.getAttribute("data-cause") ?? "";
        let nth = 0;
        for (const way of line.querySelectorAll("button")) {
          if (!waits(way, cause)) continue;
          if (!later.current.has(way)) later.current.set(way, known || held);
          // **Named by what its Notice says**, as well as its cause, label and order: a cause
          // can be one Notice's for several grants that differ only in what they name.
          names.current.set(
            way,
            `${cause}\u0000${line.textContent ?? ""}\u0000${way.textContent ?? ""}\u0000${nth}`,
          );
          nth += 1;
          yield way;
        }
      }
    },
    [memory, firstDrawn],
  );
  const remembered = useCallback(() => memory().places, [memory]);
  const recalls = useCallback(() => recalling.current, []);
  const nameOf = useCallback((way: Element) => names.current.get(way), []);
  // A way out never read (a press before its band was first drawn, #3302) is held: its first
  // drawing is a move until a read says otherwise.
  const firstMoves = useCallback(
    (way: Element) => firstDrawn === "moves" || later.current.get(way) !== false,
    [firstDrawn],
  );
  const { movedAt } = useWayPlaces(
    at,
    ways,
    firstMoves,
    {
      nameOf,
      remembered,
      recalls,
    },
    drawn,
  );
  const allows = (way: Element): boolean => {
    const notice = way.closest("[data-cause]");
    const cause = notice?.getAttribute("data-cause");
    if (!notice || cause === null || cause === undefined) return true;
    // **Any answer that grants waits**, whatever Notice it is on (#3142): the button family's
    // grants variant (a way out with `allows`) is the one rule, and the causes above add the
    // answers that grant without it.
    if (!waits(way, cause)) return true;
    if (performance.now() - movedAt(way) < SETTLE_MS) {
      setSaid(NOTICE_MOVED_JUST_NOW);
      return false;
    }
    setSaid(undefined);
    return true;
  };
  return { at, allows, said };
}
