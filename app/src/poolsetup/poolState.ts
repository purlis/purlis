// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A pool's state, as the status bar reads it** (#7191, program #7180).
 *
 * The five names come from Remote trains A, which adds them to the core. Until that enum is on
 * `main`, this file is the **one adapter**: the tab reads a pool's state only through
 * {@link poolStateOf}, so switching to the core's own field is one function here, and nothing in
 * the bar changes. It reads the `pool_state` the core sends.
 */

export const POOL_STATES = [
  "unreachable",
  "refused",
  "host-key-changed",
  "runner-gone",
  "pool-forgotten",
] as const;
export type PoolState = (typeof POOL_STATES)[number];

export type PoolStatus = {
  state: PoolState;
  /** The pool it is of: a refused add has no made pool, so the state names it. */
  pool?: string;
  /** Why, in the core's words: a refusal's reason. */
  reason?: string;
};

/** What the tab is shown, as far as the state goes: the core's field, once it has one. */
export type WithPoolState = {
  pool_state?: { state: string; pool?: string | null; reason?: string | null } | null;
};

/** **The pool's state**, or none where the core says none or one this build doesn't know. */
export function poolStateOf(shown: WithPoolState): PoolStatus | undefined {
  const said = shown.pool_state;
  if (!said) return undefined;
  const state = POOL_STATES.find((one) => one === said.state);
  if (state === undefined) return undefined;
  return {
    state,
    ...(said.pool ? { pool: said.pool } : {}),
    ...(said.reason ? { reason: said.reason } : {}),
  };
}
