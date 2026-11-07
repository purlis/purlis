// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **How long an Undo stands: a few seconds** (ADR 0065 Q8, ADR 0117 decision 9). One length for
 * every Undo the window offers: a memory's Delete or Move, a pane moved, Snooze all on Asks, an
 * ask's Ignore, and Dismiss or Dismiss all on Updates and Notices.
 */
export const UNDO_MS = 8_000;
