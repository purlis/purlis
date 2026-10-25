// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { createContext, useCallback, useRef, useState } from "react";
import { useWayPlaces } from "./wayPlaces";

/**
 * How long after a question formed, or after the tasks it lists changed, a press on it does
 * nothing: a guard against a press aimed at what was there before. It only blunts a misclick;
 * what binds an answer to what was shown is the core's check of each task's block (D-1508-9).
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
 * an ask's Allow waits on its first drawing. On a pane it does not: a Notice there arrives
 * where the person is already looking, and only a move (one above it going, say) can put a way
 * out under a pointer that was aimed at something else. A granting way out drawn into a Notice
 * already standing is a move on a pane too: it is drawn where something else was aimed at.
 */
export function useNoticeSettle<T extends HTMLElement>(firstDrawn: "moves" | "stands") {
  const at = useRef<T>(null);
  const [said, setSaid] = useState<string>();
  // The lines read so far, and for each way out whether its line was already drawn before it.
  const lines = useRef(new WeakSet<Element>());
  const later = useRef(new WeakMap<Element, boolean>());
  const ways = useCallback(function* (root: T) {
    for (const line of root.querySelectorAll("[data-cause]")) {
      const known = lines.current.has(line);
      lines.current.add(line);
      const cause = line.getAttribute("data-cause") ?? "";
      for (const way of line.querySelectorAll("button")) {
        if (!waits(way, cause)) continue;
        if (!later.current.has(way)) later.current.set(way, known);
        yield way;
      }
    }
  }, []);
  const firstMoves = useCallback(
    (way: Element) => firstDrawn === "moves" || later.current.get(way) === true,
    [firstDrawn],
  );
  const { movedAt } = useWayPlaces(at, ways, firstMoves);
  const allows = (way: Element): boolean => {
    const notice = way.closest("[data-cause]");
    const cause = notice?.getAttribute("data-cause");
    if (!notice || cause === null || cause === undefined) return true;
    // **Any answer that grants waits**, whatever Notice it is on (#3142): the button family's
    // grants variant (a way out with `allows`) is the one rule, and the causes above add the
    // answers that grant without it.
    if (!waits(way, cause)) return true;
    if (Date.now() - movedAt(way) < SETTLE_MS) {
      setSaid(NOTICE_MOVED_JUST_NOW);
      return false;
    }
    setSaid(undefined);
    return true;
  };
  return { at, allows, said };
}
