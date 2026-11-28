// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useMemo, useState } from "react";
import { dequal } from "dequal";

/**
 * **`next`, or the value of an earlier draw that says the same** (`dequal`), so a component
 * memoised on it is not drawn again for a copy (FS-W2, #7910): a list a view derives from the
 * tabs is made anew for a press on a tab, though what it lists has not moved.
 *
 * Held in state and adjusted while rendering, React's way of keeping what a render found from the
 * render before ("Storing information from previous renders"): a value that moved is held at once,
 * and this draw is made again with it before anything below it is. The comparison is kept for the
 * last input (`useMemo`), so a copy is compared once, when it arrives, and a later draw handed the
 * same copy only compares it by reference; nothing is held for an equal copy, so a caller handing
 * one on every draw costs a comparison each, never another draw.
 */
export function useHeldWhileSame<T>(next: T): T {
  const [held, hold] = useState(next);
  const same = useMemo(() => dequal(held, next), [held, next]);
  if (!same) hold(next);
  return same ? held : next;
}
