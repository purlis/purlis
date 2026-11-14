// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Open Pool setup from its ready entry** (#4496): Needs you's row asks the window to open the
 * project's Pool setup tab, where the setup goes on. Asked by an event the window hears, as the
 * other rows that open a tab are.
 */
export const OPEN_POOL_SETUP_ASK = "purlis-open-pool-setup";

/** The event the core sends when Pool setup's ready entry comes or goes (`poolsetuptab.rs`). */
export const POOL_SETUP_READY = "pool-setup-ready";

/** Asks the window to open project `plane`'s Pool setup. */
export function askOpenPoolSetup(plane: string): void {
  window.dispatchEvent(
    new CustomEvent<{ plane: string }>(OPEN_POOL_SETUP_ASK, { detail: { plane } }),
  );
}
