// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a test reads of a dialog of the family** (#2910): its answers as a person reads them,
 * in the order they stand in its footer.
 */

/** The words an element shows, without what it only keeps room for (`Reserved.tsx`). */
export function shownWords(element: Element): string {
  const copy = element.cloneNode(true) as Element;
  for (const kept of copy.querySelectorAll('[aria-hidden="true"]')) kept.remove();
  return copy.textContent ?? "";
}

/** The footer's answers of `dialog`, as their words, Cancel first and the main action last. */
export function answersOf(dialog: Element): string[] {
  const row = dialog.querySelector('[data-scope="dialog"][data-part="actions"]');
  return [...(row?.querySelectorAll("button") ?? [])].map(shownWords);
}

/** The footer's answer row of `dialog`. */
export function answerRowOf(dialog: Element): HTMLElement | null {
  return dialog.querySelector('[data-scope="dialog"][data-part="actions"]');
}
