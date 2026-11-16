// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Shown } from "./bindings";

/** Stub (T1a-61): marks nothing yet. */
export function tabsMarked(_asks: readonly Shown[], _open: readonly number[]): ReadonlySet<number> {
  return new Set();
}

/** Stub (T1a-61): where your keys locked at a start stands. */
export function unlockAskAt(_attended: boolean): { at: "new-chat" | "needs-you"; marksTab: boolean } {
  return { at: "new-chat", marksTab: true };
}
