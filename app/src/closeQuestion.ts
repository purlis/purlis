// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from "react";

import type { PlaneId } from "./bindings";

/**
 * **Whether a project's view has a close question up, as the window is told** (#2761, #2845).
 *
 * A close counts as a question from its first step (`counted`): the window is told at once,
 * before the core is asked what the close would end, so a link's close that comes meanwhile is
 * not asked over it. It stays up while any close is being asked about or a question it drew is
 * on screen (`drawn`), and is cleared once neither is so, however the close settles: each close
 * that settles makes the mark be said again, so a close that starts and settles with no render
 * in between never leaves the project marked as asking.
 */
export function useCloseQuestion(
  plane: PlaneId,
  drawn: boolean,
  onAsking: ((plane: PlaneId, up: boolean) => void) | undefined,
): { up: boolean; counted: <T>(close: () => Promise<T>) => Promise<T> } {
  /** How many closes are being asked about and have no question drawn yet, and how many have
   *  settled: a count that comes back to where it was still changes the second. */
  const [closes, setCloses] = useState({ open: 0, settled: 0 });
  const up = drawn || closes.open > 0;
  const { settled } = closes;
  useEffect(() => {
    if (onAsking === undefined) return;
    onAsking(plane, up);
    return () => onAsking(plane, false);
  }, [onAsking, plane, up, settled]);
  // `counted` is the view's own: it is never kept past the view's unmount, after which the mark
  // it sets would not be cleared.
  const counted = useCallback(
    async <T>(close: () => Promise<T>): Promise<T> => {
      onAsking?.(plane, true);
      setCloses((was) => ({ ...was, open: was.open + 1 }));
      try {
        return await close();
      } finally {
        setCloses((was) => ({ open: was.open - 1, settled: was.settled + 1 }));
      }
    },
    [onAsking, plane],
  );
  return { up, counted };
}
