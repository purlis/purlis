// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The turn's finish row in Updates › Mine** (T1-31, R-241, R-178 to R-182). A stub, so the
 * tests are seen red before the work.
 */
import type { FinishedTurn, InboxUpdate, PlaneId, UpdateNoted } from "./bindings";
import type { Chats } from "./chatState";

/** What a finish row adds where its turn ended with blocks still unresolved. */
export const BLOCKED_THIS_TURN = (blocked: number) => `${blocked}`;

/** The finish rows of `turns`. */
export function finishUpdates(
  _turns: readonly FinishedTurn[],
  _nameOf: (session: number) => string,
): UpdateNoted[] {
  return [];
}

/** Whether `update` is a finish row whose turn ended after a block. */
export const stoppedAfterBlock = (_update: Pick<InboxUpdate, "kind" | "key">): boolean => false;

/** `updates` with identical finish rows grouped. */
export function groupedFinishes(
  updates: readonly InboxUpdate[],
): { update: InboxUpdate; keys: readonly string[] }[] {
  return updates.map((update) => ({ update, keys: [update.key] }));
}

/** The finishes of every chat of `plane`, as the core counts them. */
export function useFinishedTurns(_plane: PlaneId, _chats: Chats): readonly FinishedTurn[] {
  return [];
}
