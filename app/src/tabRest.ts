// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **How long a pointer rests on a tab's chip, and how long its menu waits for it to come back**,
 * each written once. This module imports nothing, so a scenario spec reads the same numbers the
 * window runs on (`e2e/specs/tab-chip.e2e.ts`, #3660): a rest changed here is changed there, and
 * no spec is edited. `tabTasks.ts` re-exports both.
 */

/**
 * How long the pointer rests on a chip before its menu opens by itself (V100-33). A pointer
 * crossing the strip is over a chip for far less, so a pass opens nothing.
 */
export const REST_MS = 350;

/** How long the pointer may be off both the chip and a menu its rest opened before that menu
 *  closes: the time to cross the gap between them, and to come back from a slip off the edge. */
export const GRACE_MS = 300;
