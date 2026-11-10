// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState } from "react";
import type { Shown } from "./bindings";
import { chainSaid } from "./inboxRowParts";
import { askKey } from "./inboxRules";

/**
 * **"The ✋ has pulsed" on this machine** (ADR 0121 decision 24): kept in this window's web
 * storage, as the card's harness note is (`cardLink.tsx`, D-1134-3). A per-viewer convenience:
 * storage lost or unreadable costs one more pulse, and a record in a file purlis keeps would be a
 * format key nothing decided.
 */
export const PULSED_KEY = "purlis.asks-pulsed";

/** Whether this machine's ✋ has pulsed; storage that cannot say reads as not. */
function pulsedHere(): boolean {
  try {
    return globalThis.localStorage?.getItem(PULSED_KEY) !== null;
  } catch {
    return false;
  }
}

/** Keeps that the ✋ has pulsed, so it never pulses again on this machine. */
function notePulsed(): void {
  try {
    globalThis.localStorage?.setItem(PULSED_KEY, new Date().toISOString());
  } catch {
    // Not kept: it pulses once more, which is all lost storage costs.
  }
}

/**
 * **What a screen reader is told when asks arrive** (ADR 0121 decision 25): the chat and what it
 * asks, where one arrived here; otherwise how many. What an ask says is data, read as it is.
 */
export function arrivedSaid(arrived: readonly Shown[], elsewhere: number): string {
  const count = arrived.length + elsewhere;
  if (count > 1) return `${count} new asks`;
  const [one] = arrived;
  if (one === undefined) return "New ask in another project";
  return `New ask from ${chainSaid(one.chain)}: ${one.says}`;
}

/** A line to be said, and how many have been: a new count is a new line, said once. */
export type ArrivalSaid = { said: string; count: number };

/**
 * **What the window does as a Blocking ask arrives** (ADR 0121 decisions 24 and 25, #6020).
 *
 * - **Said once, politely**: `said` changes only when an ask arrives that was not listed the
 *   last time the asks were read, here (`asks`) or elsewhere (`elsewhere`, a count). Drawing the
 *   same asks again says nothing. What already waited when the asks were first read is not news.
 * - **The first pulse**: the first ask on this machine to arrive while Asks is closed (`shown`
 *   false) sets `pulse`, once, and that is kept per machine. `pulsed` is told when the ✋'s
 *   animation ends (reduced motion still ends it, at once); opening Asks stops it too.
 */
export function useAsksArrival(
  asks: readonly Shown[] | undefined,
  elsewhere: number,
  shown: boolean,
): { said: ArrivalSaid; pulse: boolean; pulsed: () => void } {
  const known = useRef<{ keys: ReadonlySet<string>; elsewhere: number }>(undefined);
  const [said, setSaid] = useState<ArrivalSaid>({ said: "", count: 0 });
  const [pulse, setPulse] = useState(false);
  useEffect(() => {
    if (asks === undefined) return;
    const was = known.current;
    known.current = { keys: new Set(asks.map(askKey)), elsewhere };
    if (was === undefined) return;
    const arrived = asks.filter((one) => !was.keys.has(askKey(one)));
    const rose = Math.max(0, elsewhere - was.elsewhere);
    if (arrived.length === 0 && rose === 0) return;
    setSaid((before) => ({ said: arrivedSaid(arrived, rose), count: before.count + 1 }));
    if (!shown && !pulsedHere()) {
      notePulsed();
      setPulse(true);
    }
  }, [asks, elsewhere, shown]);
  // Opening Asks ends the pulse for good, so closing it again does not play it a second time:
  // adjusted as it is drawn, as React has state that follows a prop.
  if (pulse && shown) setPulse(false);
  const pulsed = useCallback(() => setPulse(false), []);
  return { said, pulse, pulsed };
}
