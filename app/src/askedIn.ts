// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **How many Blocking asks wait in each open project of every window** (#6341, ADR 0121 amended
 * 2026-11-12, decision 1): the one count the ✋ reads for the project in front, each project tab
 * on the rail reads for its own, and the Dock badge adds up (`dockBadge.dockTotal`).
 *
 * - **This window's projects** count their asks registry's asks, and, where it has said nothing
 *   yet, their chats in the queue, as the ✋ always did.
 * - **Another window's project** counts the asks that window told (`WindowSaid.asks`, #1921),
 *   and one for each chat it says waits where it told no asks.
 *
 * A project of this window is counted once, here, even while a window it moved from still tells
 * it. A snoozed chat's asks are in none (`Asking.snoozed`).
 */
export function askedIn({
  planes,
  held,
  needing,
  there,
  asking,
}: {
  /** This window's projects. */
  planes: readonly string[];
  /** Each of this window's projects' asks, as its registry derived them; absent until read. */
  held: Readonly<Record<string, readonly unknown[] | undefined>>;
  /** This window's chats in the queue. */
  needing: readonly { plane: string }[];
  /** The other windows' projects whose asks they told, with how many. */
  there: readonly { plane: string; count: number }[];
  /** The chats the other windows say wait. */
  asking: readonly { plane: string }[];
}): ReadonlyMap<string, number> {
  const counts = new Map<string, number>();
  for (const plane of planes)
    counts.set(plane, held[plane]?.length ?? needing.filter((one) => one.plane === plane).length);
  for (const one of there) if (!counts.has(one.plane)) counts.set(one.plane, one.count);
  const told = new Set(counts.keys());
  for (const one of asking)
    if (!told.has(one.plane)) counts.set(one.plane, (counts.get(one.plane) ?? 0) + 1);
  return counts;
}

/** The ✋'s number: the asks of the project in front alone, and none with no project in front. */
export const handOf = (asked: ReadonlyMap<string, number>, inFront: string | undefined): number =>
  inFront === undefined ? 0 : (asked.get(inFront) ?? 0);
