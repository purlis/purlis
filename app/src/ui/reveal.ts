// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Bring a control out of a fold before the keyboard goes to it** (#3675): a field a form
 * refused, inside a folded disclosure, cannot take the focus while it is hidden. The form says
 * so to every fold around it, which opens there and then, so the focus that follows lands.
 *
 * An event, not a context, so a form never needs to know which folds stand between it and its
 * fields: each disclosure listens on its own root, and the event bubbles through every one.
 */

/** The event a fold opens on: sent at the control, bubbling through every fold around it. */
export const REVEAL_EVENT = "purlis:reveal";

/** Opens every fold around `element`, at once, so it can take the focus straight after. */
export function reveal(element: Element): void {
  element.dispatchEvent(new Event(REVEAL_EVENT, { bubbles: true }));
}
