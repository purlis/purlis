// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { createContext, useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";

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

const now = () => Date.now();

/**
 * **What a press on a Notice's way out asks first** (#1695, #2904): whether the place it is
 * drawn in lets it act now. A list of Notices that come and go provides one ({@link
 * useNoticeSettle}'s `allows`); a Notice standing alone acts at once. It is asked in the press
 * itself, so a press from the keyboard waits as one from the pointer does.
 */
export const NoticeWayGuard = createContext<(way: Element) => boolean>(() => true);

/**
 * **The Notices' move-settle guard** (#1695, train 50's and train 54's reviews): a Notice that
 * grants something is drawn among others that come and go, so a press on one of its ways out
 * within {@link SETTLE_MS} of it moving does nothing, and says why. Where each Notice stands is
 * read after every change under `at` and again at the press: one that differs is a move.
 *
 * **Where a Notice stands is read against the root itself**, never against the Notice's
 * `offsetParent`: a box drawn with a position of its own (a pane warning's mark, #3142) would make
 * itself that parent, and then a Notice going or arriving above another would read as no move
 * at all. The place is the line's top less the root's, with what the boxes between them have
 * scrolled added back, so scrolling a list is not a move and a layout change is.
 *
 * `firstDrawn` says whether a Notice's first drawing counts as a move. In the Inbox it does, as
 * an ask's Allow waits on its first drawing. On a pane it does not: a Notice there arrives
 * where the person is already looking, and only a move (one above it going, say) can put a way
 * out under a pointer that was aimed at something else.
 */
export function useNoticeSettle<T extends HTMLElement>(firstDrawn: "moves" | "stands") {
  const at = useRef<T>(null);
  const placed = useRef(new WeakMap<Element, { top: number; at: number }>());
  const [said, setSaid] = useState<string>();
  const look = useCallback(
    (one: Element) => {
      const root = at.current;
      let top = one.getBoundingClientRect().top - (root?.getBoundingClientRect().top ?? 0);
      for (let el = one.parentElement; el !== null && el !== root; el = el.parentElement)
        top += el.scrollTop;
      const was = placed.current.get(one);
      if (was === undefined) {
        placed.current.set(one, { top, at: firstDrawn === "moves" ? now() : -Infinity });
      } else if (was.top !== top) {
        placed.current.set(one, { top, at: now() });
      }
      return placed.current.get(one)?.at ?? now();
    },
    [firstDrawn],
  );
  const lookAll = useCallback(() => {
    for (const one of at.current?.querySelectorAll("[data-cause]") ?? []) look(one);
  }, [look]);
  useLayoutEffect(() => {
    lookAll();
  });
  useEffect(() => {
    const root = at.current;
    if (root === null || typeof MutationObserver === "undefined") return;
    const watching = new MutationObserver(lookAll);
    watching.observe(root, { childList: true, subtree: true });
    return () => watching.disconnect();
  }, [lookAll]);
  const allows = (way: Element): boolean => {
    const notice = way.closest("[data-cause]");
    const cause = notice?.getAttribute("data-cause");
    if (!notice || cause === null || cause === undefined || !grants(cause)) return true;
    if (now() - look(notice) < SETTLE_MS) {
      setSaid(NOTICE_MOVED_JUST_NOW);
      return false;
    }
    setSaid(undefined);
    return true;
  };
  return { at, allows, said };
}
