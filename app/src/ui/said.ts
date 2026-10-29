// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a value that changes says to a reader, at most once a second** (#3673, #3674): a
 * running grade's counts in its totals tiles, a progress bar's done of N. Each change is said
 * politely (a `role="status"` region the family draws), and never more often than this, so a
 * reader hears the counts without being flooded.
 */
import { useEffect, useRef, useState } from "react";

/** **How often live totals may speak**, in ms: at most once a second. */
export const SAID_AT_MOST_EVERY = 1000;

/**
 * **What `said` says, at most once every `every` ms**: a change at once where nothing was said
 * within that time, else once it is over, and then only the last of several. The words it is
 * first given are no change, and are not said.
 */
export function useSaidAtMost(said: string, every = SAID_AT_MOST_EVERY): string {
  const [spoken, setSpoken] = useState("");
  const last = useRef(Number.NEGATIVE_INFINITY);
  // What was there before, and whether any words have been given yet: the first words, as the
  // bar or the row is first drawn with them, are no change and are not spoken. Words after a
  // time with none (a bar that was going with no end known) are.
  const before = useRef("");
  const given = useRef(false);
  useEffect(() => {
    if (said === before.current) return;
    before.current = said;
    if (said === "") return;
    if (!given.current) {
      given.current = true;
      return;
    }
    const soon = window.setTimeout(
      () => {
        last.current = Date.now();
        setSpoken(said);
      },
      Math.max(0, last.current + every - Date.now()),
    );
    return () => window.clearTimeout(soon);
  }, [said, every]);
  return spoken;
}
