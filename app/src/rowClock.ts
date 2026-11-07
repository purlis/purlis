// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";
import { everyWhileShown } from "./whileShown";

/**
 * **The window's one clock for what a row says in minutes** (#4564, #3032): a chat row's time
 * in its state (`stateClock.ts`), an Inbox row's or a Notice's age (`InboxRow.tsx`). It imports
 * nothing of the person's state or the core, so a part that draws an age with no window around
 * it (the theme specimen) can use it.
 */

/** How often a row's time is read again. A row says minutes, so half of one is enough. It is
 *  read on the window's one beat (`whileShown.ts`): not while the window is hidden, and at once
 *  when it is shown again, so a row is never drawn with the minutes it had when the window went. */
const TICK_MS = 30_000;
let nowMs = 0;
/** Whether `nowMs` was read since the clock last stopped: a row drawn before the clock starts
 *  draws the time it starts with, and is not drawn again for the start. */
let nowRead = false;
let stopTicking: (() => void) | undefined;
const tickers = new Set<() => void>();

function tick(): void {
  nowMs = Date.now();
  for (const one of [...tickers]) one();
}

/** The clock's time, read once when it is first asked for after the clock stopped. */
function nowAsRead(): number {
  if (!nowRead) {
    nowMs = Date.now();
    nowRead = true;
  }
  return nowMs;
}

function everyTick(listener: () => void): () => void {
  tickers.add(listener);
  if (stopTicking === undefined) {
    nowAsRead();
    stopTicking = everyWhileShown(TICK_MS, tick);
  }
  return () => {
    tickers.delete(listener);
    if (tickers.size === 0) {
      stopTicking?.();
      stopTicking = undefined;
      nowRead = false;
    }
  };
}

/**
 * **The row's clock, now** (#4564): the same one beat every row's time is read on, twice a
 * minute, so what a row says in minutes changes at most once a minute.
 */
export function useNow(): number {
  return useSyncExternalStore(everyTick, nowAsRead, nowAsRead);
}
