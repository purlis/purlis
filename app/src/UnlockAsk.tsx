// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Unlocked } from "./bindings";

/** Stub (T1a-61): draws nothing yet. */
export function UnlockAsk(_props: {
  entry: string;
  unlock: () => Promise<Unlocked>;
  onUnlocked: () => void;
}) {
  return null;
}
