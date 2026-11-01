// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { commands, type WatchedBranch } from "./bindings";

/**
 * **The branches this window listens to, whoever wants each.** The core keeps one set per window
 * and each `branch_watch` replaces it, so the explorer and the Skills view (#4377) would each
 * replace the other's: the core is told their union, as `branchFolders.ts` tells it folders.
 */
const wantedBy = new Map<symbol, readonly WatchedBranch[]>();
/** Whether the core listens to anything for this window, so a window that never wanted a branch
 *  never asks it to listen to nothing. */
let listening = false;
let newest: Promise<unknown> = Promise.resolve();

/**
 * Sets what `owner` wants listened to, none to let go. Settles once the newest set sent by then,
 * from any owner, has been answered: the core passes over an older set.
 */
export function listenToBranches(
  owner: symbol,
  branches: readonly WatchedBranch[],
): Promise<unknown> {
  if (branches.length === 0) wantedBy.delete(owner);
  else wantedBy.set(owner, branches);
  const union = new Map<string, WatchedBranch>();
  for (const one of [...wantedBy.values()].flat()) {
    union.set(`${one.plane}\0${one.workspace}\0${one.repo}\0${one.piece ?? ""}`, one);
  }
  const all = [...union.values()];
  if (all.length === 0 && !listening) return newest;
  listening = all.length > 0;
  newest = commands.branchWatch(all).catch(() => undefined);
  return newest;
}
