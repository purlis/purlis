// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A text box in a row keeps its own keys** (#3123): a list of rows that act (`List`, React
 * Aria's `GridList`) takes ← and → to walk a row's controls, ↑ and ↓, Home and End, PageUp and
 * PageDown to move between rows, a letter to find one, and Enter and Space to run or choose the
 * row. In a text box in a row (an Inbox ask's reply, its reason) each of those is the box's: they
 * move its caret and type its words. So:
 *
 * - **← and →** move the caret, and never walk the row's other controls;
 * - **↑ and ↓** move the caret between a box's lines, and go to the row above or below from its
 *   first or last line, or from a box of one line;
 * - **Home, End, PageUp, PageDown, a letter, Space and Enter, and ⌘A or Ctrl+A** are the box's:
 *   the box's own handler hears them, and the row and the list never do (a letter finds no row,
 *   Enter runs none). A chord with Enter (⌘⏎, a form's send) goes on up to whatever answers it;
 * - **Tab** moves on to the row's next control while it has one, and Shift+Tab back to the one
 *   before; from its last, or first, it leaves the list as Tab leaves any;
 * - **Esc** gives the keyboard back to its row, keeping what was typed; Esc on the row is the
 *   list's (`onEscape`).
 *
 * Two halves, because React Aria hears them in two places. A row hears the arrows on their way
 * down to the box (a capture handler), so the list hears them first, around it, and keeps them
 * from it: the browser still moves the caret, and nobody's handler hears them
 * ({@link keptOnTheWayDown}). The rest a row and its list hear on their way back up, so the field
 * family's boxes keep them once their own handler has heard them ({@link keptOnTheWayUp}).
 */
import type { KeyboardEvent } from "react";

/** The input types that are not a box words are typed in. */
const NOT_TYPED = new Set([
  "button",
  "checkbox",
  "color",
  "file",
  "hidden",
  "image",
  "radio",
  "range",
  "reset",
  "submit",
]);

/** Whether `element` is a box words are typed in: a text field, a box of lines, an editor. */
export function isTextBox(element: EventTarget | null): element is HTMLElement {
  if (element instanceof HTMLTextAreaElement) return true;
  if (element instanceof HTMLInputElement) return !NOT_TYPED.has(element.type);
  return element instanceof HTMLElement && element.isContentEditable;
}

/** The row of a list of rows that act that `box` stands in, or none. */
function rowOf(box: HTMLElement): HTMLElement | null {
  const row = box.closest<HTMLElement>('[role="row"]');
  if (row === null || row === box) return null;
  return row.closest('[role="grid"][data-scope="list"]') === null ? null : row;
}

/** Whether the caret can move a line up (`up`) or down inside `box`. */
function hasLineBeyond(box: HTMLElement, up: boolean): boolean {
  if (box instanceof HTMLInputElement) return false;
  if (!(box instanceof HTMLTextAreaElement)) return true;
  const { value, selectionStart, selectionEnd } = box;
  return up
    ? value.lastIndexOf("\n", selectionStart - 1) !== -1
    : value.indexOf("\n", selectionEnd) !== -1;
}

/**
 * **Whether the list keeps `event` from its rows on its way down** (a capture handler around
 * the list): an arrow a text box in a row moves its caret with. Esc in such a box is kept from
 * the list's own Esc, so the box gives the keyboard back to its row instead.
 */
export function keptOnTheWayDown(event: KeyboardEvent<HTMLElement>): boolean {
  const box = event.target;
  if (!isTextBox(box) || rowOf(box) === null) return false;
  switch (event.key) {
    case "ArrowLeft":
    case "ArrowRight":
      return true;
    case "ArrowUp":
      return hasLineBeyond(box, true);
    case "ArrowDown":
      return hasLineBeyond(box, false);
    default:
      return false;
  }
}

/** Whether Esc on `event`'s target is a text box's in a row, which gives it back to its row. */
export function isEscapeInABox(event: KeyboardEvent<HTMLElement>): boolean {
  return event.key === "Escape" && isTextBox(event.target) && rowOf(event.target) !== null;
}

/** The keys a list moves between its rows with that a box moves its caret with, chorded or not. */
const CARET = new Set(["Home", "End", "PageUp", "PageDown"]);

/** The controls the keyboard reaches by Tab inside `row`, in their order. */
function tabbable(row: HTMLElement): HTMLElement[] {
  return [
    ...row.querySelectorAll<HTMLElement>(
      'button, input, textarea, select, a[href], [tabindex], [contenteditable="true"]',
    ),
  ].filter((one) => one.tabIndex >= 0 && !one.hasAttribute("disabled") && !one.inert);
}

/**
 * **What a text box in a row keeps on the key's way back up** (the field family's boxes call it
 * after their own handler): the keys its row or list would otherwise act on.
 */
export function keptOnTheWayUp(event: KeyboardEvent<HTMLElement>): void {
  const box = event.currentTarget;
  if (event.isPropagationStopped() || event.target !== box) return;
  const row = rowOf(box);
  if (row === null) return;
  if (event.key === "Escape") {
    if (event.defaultPrevented || event.nativeEvent.isComposing) return;
    event.preventDefault();
    event.stopPropagation();
    row.focus();
    return;
  }
  if (event.key === "Tab") {
    const order = tabbable(row);
    const at = order.indexOf(box);
    const next = order[event.shiftKey ? at - 1 : at + 1];
    // The browser moves to it, where the list would take the keyboard out of itself.
    if (at !== -1 && next !== undefined) event.stopPropagation();
    return;
  }
  const chord = event.ctrlKey || event.metaKey || event.altKey;
  const typed = event.key.length === 1 && !chord;
  const selectsAll = event.key.toLowerCase() === "a" && (event.ctrlKey || event.metaKey);
  // A chord goes on up (⌘⏎ sends a form, `submitKey.ts`): only Enter and Space typed alone, or
  // with Shift, are the box's.
  const pressed = (event.key === "Enter" || event.key === " ") && !chord;
  if (CARET.has(event.key) || pressed || typed || selectsAll) event.stopPropagation();
}
