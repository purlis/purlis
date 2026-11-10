// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useSyncExternalStore } from "react";
import { standingSaid, type Standing } from "./asksOrder";
import { MessageMeta } from "./ui/message/Message";
import { everyWhileShown } from "./whileShown";

/** How often an Expiring ask's countdown is read again: it says seconds. */
const COUNTDOWN_MS = 1_000;

/** Reads `Date.now()` on the window's one-second beat while the window is shown. */
const everySecond = (changed: () => void) => everyWhileShown(COUNTDOWN_MS, changed);

/** The countdown's time, to the second, so the store reads the same between beats. */
const thisSecond = () => Math.floor(Date.now() / COUNTDOWN_MS) * COUNTDOWN_MS;

/**
 * **A row's reason line** (ADR 0121 decision 5, #6006): why the ask stands where it does in
 * Asks, in words (`standingSaid`). An Expiring ask's counts down on a clock read every second
 * while the window is shown; every other reads the rows' clock, `now`, and beats on no clock of
 * its own.
 */
export function AskStanding({ why, now, id }: { why: Standing; now: number; id?: string }) {
  const counting = why.by === "expires";
  const subscribe = useCallback(
    (changed: () => void) => (counting ? everySecond(changed) : () => {}),
    [counting],
  );
  const second = useSyncExternalStore(subscribe, thisSecond, thisSecond);
  return (
    <MessageMeta id={id}>
      <span className="inbox-standing" data-standing={why.by}>
        {standingSaid(why, counting ? second : now)}
      </span>
    </MessageMeta>
  );
}
