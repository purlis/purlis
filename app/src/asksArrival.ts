// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import type { Shown } from "./bindings";
import { chainSaid } from "./inboxRowParts";
import { askKey } from "./inboxRules";

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
 * **What the window does as a Blocking ask arrives** (ADR 0121 decision 25, #6020).
 *
 * - **Said once, politely**: `said` changes only when an ask arrives that was not listed the
 *   last time the asks were read, here (`asks`) or elsewhere (`elsewhere`, a count). Drawing the
 *   same asks again says nothing. What already waited when the asks were first read is not news.
 * - **`here` counts the arrivals in this project**, each read that brought one or more: what
 *   bounces the ✋ (`useHandBounce`, #6343). One in another project is said, and does not count.
 */
export function useAsksArrival(
  asks: readonly Shown[] | undefined,
  elsewhere: number,
): { said: ArrivalSaid; here: number } {
  const known = useRef<{ keys: ReadonlySet<string>; elsewhere: number }>(undefined);
  const [said, setSaid] = useState<ArrivalSaid>({ said: "", count: 0 });
  const [here, setHere] = useState(0);
  useEffect(() => {
    if (asks === undefined) return;
    const was = known.current;
    known.current = { keys: new Set(asks.map(askKey)), elsewhere };
    if (was === undefined) return;
    const arrived = asks.filter((one) => !was.keys.has(askKey(one)));
    const rose = Math.max(0, elsewhere - was.elsewhere);
    if (arrived.length === 0 && rose === 0) return;
    setSaid((before) => ({ said: arrivedSaid(arrived, rose), count: before.count + 1 }));
    if (arrived.length > 0) setHere((n) => n + 1);
  }, [asks, elsewhere]);
  return { said, here };
}
