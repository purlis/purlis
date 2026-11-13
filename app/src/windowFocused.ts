// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import { afterPaint, useOnShownAgain, useProjectShown } from "./projectShown";

/**
 * **How many times the window has come back into focus** since the caller mounted: a
 * dependency for a read that should follow what no watcher reports, such as a file another
 * window or a hand wrote, or a pull made while the window was away (#1407). Starts at 0 and
 * moves by one on each `focus` of the window.
 *
 * **Counted only while the caller's project is in front** (`projectShown.tsx`): a project that
 * stays drawn behind another reads nothing for a window whose focus is on something else. **And
 * once more as it comes back**, after it is painted: coming back to a project is coming back to
 * what a hand or another window may have written meanwhile, as coming back to the window is.
 */
export function useWindowFocused(): number {
  const [focused, setFocused] = useState(0);
  const shown = useProjectShown();
  useEffect(() => {
    if (!shown) return;
    const again = () => setFocused((n) => n + 1);
    window.addEventListener("focus", again);
    return () => window.removeEventListener("focus", again);
  }, [shown]);
  useOnShownAgain(() => setFocused((n) => n + 1));
  return focused;
}

/**
 * **Calls `run` where {@link useWindowFocused} would count, and draws nothing itself** (felt
 * speed, round 3, H7): on each `focus` of the window while `shown`, and once as `shown` comes
 * back, after the paint. For a hook the project's view calls itself: that view sits above its
 * own `ProjectShownProvider`, so it is handed whether it is in front, and a count kept in its
 * state drew the whole view, in every project the window keeps drawn, on every focus. The
 * latest `run` is the one called.
 */
export function useOnWindowFocus(shown: boolean, run: () => void): void {
  const latest = useRef(run);
  useEffect(() => {
    latest.current = run;
  });
  const was = useRef(shown);
  useEffect(() => {
    const before = was.current;
    was.current = shown;
    if (!shown) return;
    const again = () => latest.current();
    window.addEventListener("focus", again);
    const stopAfterPaint = before ? undefined : afterPaint(again);
    return () => {
      window.removeEventListener("focus", again);
      stopAfterPaint?.();
    };
  }, [shown]);
}
