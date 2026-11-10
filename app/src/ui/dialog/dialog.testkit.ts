// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a test reads of a dialog of the family** (#2910): its answers as a person reads them,
 * in the order they stand in its footer.
 */

import { fireEvent } from "@testing-library/react";

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

/**
 * What takes Tab in a dialog: the family's own (`useTabbing` in `Dialog.tsx`, its stop from
 * `tabOrder.ts`), or, in a surface with an anatomy of its own (the palette), React Aria's
 * `FocusScope`.
 */
export type TabTakenBy = "the dialog family" | "the focus scope";

/**
 * **Presses Tab (Shift+Tab with `shift`) on what has the keyboard, and fails unless `by` took
 * it** (#6069). Whatever takes it moves the focus itself and prevents the default; a press that
 * comes back unprevented was left to the engine, which jsdom does not run.
 *
 * **Which of the two took it is told by whether the key reached the control.** React Aria's
 * dialog asks its `Overlay` for a `FocusScope` that contains (`useDialog`), which hears the key on
 * the document as it bubbles and wraps round the dialog as well, so a test that only watched the
 * focus would pass with the family's Tab gone. The family hears the key first, on the window as
 * it is captured, and stops it there: it never reaches the focused control. In the palette the
 * family stands back, and the key reaches the box before the scope takes it.
 */
export function pressTab(shift = false, by: TabTakenBy = "the dialog family"): void {
  const from = document.activeElement;
  if (
    !(from instanceof HTMLElement) ||
    from.closest('[role="dialog"], [role="alertdialog"]') === null
  )
    throw new Error("Tab was pressed with the keyboard outside a dialog");
  const what = from.getAttribute("aria-label") ?? from.textContent?.trim() ?? from.localName;
  let reached = false;
  const heard = () => {
    reached = true;
  };
  from.addEventListener("keydown", heard);
  const unprevented = fireEvent.keyDown(from, { key: "Tab", shiftKey: shift });
  from.removeEventListener("keydown", heard);
  if (unprevented) throw new Error(`Tab from "${what}" was left to the engine: nothing took it`);
  if (reached !== (by === "the focus scope"))
    throw new Error(`Tab from "${what}" was taken, but not by ${by}`);
}
