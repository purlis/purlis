// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Review… on a chat's proposed pin** (#5963): it asks the project's window to open Pin to
 * workspace on the proposal (`ProposedPin.tsx`). A Notice on a pane and an Inbox row are drawn
 * far from the window's dialogs, so they ask by an event the window hears, as Open on a proposed
 * change does (`proposedChangeOpen.ts`).
 */
export const REVIEW_PIN_ASK = "purlis-review-proposed-pin";

/** What a row asked of a project's window: which project, and which proposal. */
export type ReviewPinAsk = { plane: string; id: number };

/** Asks the window of project `plane` to open proposal `id`'s Pin to workspace. */
export function askReviewPin(plane: string, id: number): void {
  window.dispatchEvent(new CustomEvent<ReviewPinAsk>(REVIEW_PIN_ASK, { detail: { plane, id } }));
}
