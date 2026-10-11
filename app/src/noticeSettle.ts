import { useCallback, useEffect, useLayoutEffect, useRef, useState, type MouseEvent } from "react";
import { SETTLE_MS } from "./TaskBlocksNotice";

/** What a press on a granting Notice drawn or moved just now says: nothing was done. */
export const NOTICE_MOVED_JUST_NOW =
  "That Notice was drawn or moved just now, so nothing was done. Read it and press again.";

/**
 * **Whether a Notice grants something** (#1695): its ways out allow a host for a persona or a
 * chat, accept a teammate's dispatch grant, allow a dispatch, or answer a task's permission
 * prompt. Its presses wait for it to settle where it is drawn.
 */
export const grants = (cause: string) =>
  (cause.startsWith("sandbox-blocked:") && !cause.endsWith(":allowed")) ||
  cause.startsWith("sandbox-hosts:persona:") ||
  cause.startsWith("task-permission:") ||
  cause === "dispatch-grants" ||
  cause.startsWith("dispatch-grant:");

const now = () => Date.now();

/**
 * **The Notices' move-settle guard** (#1695, train 50's and train 54's reviews): a Notice that
 * grants something is drawn among others that come and go, so a press on one of its ways out
 * within {@link SETTLE_MS} of it moving does nothing, and says why. Where each Notice stands is
 * read after every change under `at` and again at the press: one that differs is a move.
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
      const top = (one as HTMLElement).offsetTop ?? 0;
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
  const guard = (event: MouseEvent) => {
    const way = (event.target as Element).closest("button.notice-fix");
    const notice = way?.closest("[data-cause]");
    const cause = notice?.getAttribute("data-cause");
    if (!way || !notice || cause === null || cause === undefined || !grants(cause)) return;
    if (now() - look(notice) < SETTLE_MS) {
      event.preventDefault();
      event.stopPropagation();
      setSaid(NOTICE_MOVED_JUST_NOW);
      return;
    }
    setSaid(undefined);
  };
  return { at, guard, said };
}
