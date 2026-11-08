// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { NotChecked } from "./bindings";

/**
 * **What the window says of an older runner purlis keeps unchecked** (the person's ruling of
 * 2026-11-08): a draining runner that could not be checked for work only it holds, because it had
 * stopped before its drain or because its container was removed outside purlis and only its saved
 * home is left. Each line names the runner and what is left of it, what purlis does (checks it
 * once it runs again, or cannot check it at all) and what the person can do: start it, or let it
 * go and lose whatever it holds. Written once here, so Settings › Pools, Devices and the ask
 * before Let it go say the same thing.
 */

/** The Pools group's line for runner `runner`, one line each. */
export function notCheckedPoolSaid(runner: string, left: NotChecked): string {
  return left === "stopped"
    ? `Older runner ${runner} stays, since it stopped before purlis could check it for work only it holds. Its stopped container is still in Docker. Start it there and purlis checks it, or let it go in Devices, which deletes whatever it holds.`
    : `Older runner ${runner} stays, since only its saved home is left: its container was removed outside purlis, so purlis can't check the home for work only it holds. Let it go in Devices to delete the home, with whatever it holds.`;
}

/** The Devices Notice's line for runner `runner` of pool `pool`. */
export function notCheckedNoticeSaid(runner: string, pool: string, left: NotChecked): string {
  return left === "stopped"
    ? `purlis keeps older runner ${runner} of pool ${pool}, since it stopped before purlis could check it for work only it holds. Its stopped container is still in Docker. Start it there and purlis checks it, or let it go with whatever it holds.`
    : `purlis keeps the saved home of older runner ${runner} of pool ${pool}. Its container was removed outside purlis, so purlis can't check the home for work only it holds. Let it go to delete the home, with whatever it holds.`;
}

/** The runner's fact on its Devices row. */
export function notCheckedFact(left: NotChecked): string {
  return left === "stopped"
    ? "An older runner purlis keeps, since it stopped before purlis could check it"
    : "An older runner whose saved home purlis keeps, since purlis can't check it";
}

/** What Let it go removes, and that what it holds is lost unseen, as the ask says it. */
export function notCheckedLost(left: NotChecked): string {
  return left === "stopped"
    ? "Its stopped container and its home are removed from your Docker. purlis never checked them, so any work only they hold is lost with them."
    : "Its saved home is removed from your Docker. purlis could not check it, so any work only it holds is lost with it.";
}
