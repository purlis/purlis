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
 *   list's (`onEscape`);
 * - **every key while it composes** (Chinese, Japanese, Korean input) is the input method's
 *   alone (#5993): Esc cancels the composition and the keyboard stays in the box, ↓ picks a
 *   candidate and the list stays on its row ({@link isComposing}).
 *
 * ↑ and ↓ go by the lines the box draws, a line its words wrap on to as much as one after a line
 * break ({@link hasLineBeyond}).
 *
 * A radio group in a row keeps its arrows the same way ({@link movedInARadioGroup}), and a
 * select or a combobox keeps ↑ and ↓ to open with ({@link keepsItsArrows}).
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

/**
 * **Whether a key is the input method's** (#5993): pressed while a composition is open, or the
 * one that ends it. WebKit ends the composition before the key's `keydown`, which then says it
 * is not composing, and tells it only by its key code, 229, as every browser does for a key the
 * input method takes.
 */
export function isComposing(event: KeyboardEvent<HTMLElement>): boolean {
  return event.nativeEvent.isComposing || event.nativeEvent.keyCode === 229;
}

/**
 * Whether the caret can move a line up (`up`) or down inside `box`: past a line break, or on to
 * a line the box's words wrap on to (#5993).
 */
function hasLineBeyond(box: HTMLElement, up: boolean): boolean {
  if (box instanceof HTMLInputElement) return false;
  if (!(box instanceof HTMLTextAreaElement)) return true;
  const { value, selectionStart, selectionEnd } = box;
  const broken = up
    ? value.lastIndexOf("\n", selectionStart - 1) !== -1
    : value.indexOf("\n", selectionEnd) !== -1;
  return broken || wrapsBeyond(box, up ? selectionStart : selectionEnd, up);
}

/** The box's styles that say where its words wrap, which its measure copies. */
const WRAPPING = [
  "direction",
  "fontFamily",
  "fontFeatureSettings",
  "fontKerning",
  "fontSize",
  "fontStretch",
  "fontStyle",
  "fontVariant",
  "fontWeight",
  "letterSpacing",
  "lineHeight",
  "overflowWrap",
  "paddingLeft",
  "paddingRight",
  "tabSize",
  "textIndent",
  "textTransform",
  "whiteSpace",
  "wordBreak",
  "wordSpacing",
] as const;

/**
 * **Whether the caret at `at` has a line its words wrap on to above it (`up`) or below it**, in
 * the line between line breaks it stands in. A box of lines tells no line its caret is on, so its
 * words are laid out again in a measure drawn as the box is (its width, its font, its wrapping),
 * unseen: the words before the caret and the words after it, each its own run, whose lines the
 * browser tells (`getClientRects`). The caret is on the first line of the words after it, or the
 * last of those before it at the end; it has a line above while that is below the first run's
 * first line, and one below while the words after it go on to a line further down.
 */
function wrapsBeyond(box: HTMLTextAreaElement, at: number, up: boolean): boolean {
  const { value } = box;
  const from = value.lastIndexOf("\n", at - 1) + 1;
  const next = value.indexOf("\n", at);
  const to = next === -1 ? value.length : next;
  const before = value.slice(from, at);
  const after = value.slice(at, to);
  if (up ? before === "" : after === "") return false;
  const document = box.ownerDocument;
  const measure = document.createElement("div");
  const style = getComputedStyle(box);
  for (const one of WRAPPING) measure.style[one] = style[one];
  Object.assign(measure.style, {
    position: "absolute",
    top: "0",
    left: "-9999px",
    visibility: "hidden",
    boxSizing: "border-box",
    width: `${box.clientWidth}px`,
    border: "0",
  });
  const run = (words: string) => {
    const span = document.createElement("span");
    span.textContent = words;
    measure.append(span);
    return span;
  };
  const ahead = run(before);
  const behind = run(after);
  document.body.append(measure);
  try {
    const first = (span: HTMLElement) => span.getClientRects()[0]?.top;
    const last = (span: HTMLElement) => {
      const rects = span.getClientRects();
      return rects[rects.length - 1]?.top;
    };
    const caret = after === "" ? last(ahead) : first(behind);
    const edge = up ? first(ahead) : last(behind);
    if (caret === undefined || edge === undefined) return false;
    return up ? edge < caret - 1 : edge > caret + 1;
  } finally {
    measure.remove();
  }
}

/**
 * **Whether the list keeps `event` from its rows on its way down** (a capture handler around
 * the list): an arrow a text box in a row moves its caret with, and any key while it composes.
 * Esc in such a box is kept from the list's own Esc, so the box gives the keyboard back to its
 * row instead.
 */
export function keptOnTheWayDown(event: KeyboardEvent<HTMLElement>): boolean {
  const box = event.target;
  if (!isTextBox(box) || rowOf(box) === null) return false;
  // Composing, every key is the input method's: nothing in the row or above it hears one.
  if (isComposing(event)) return true;
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
    if (event.defaultPrevented || isComposing(event)) return;
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

/**
 * **A select or a combobox in a row keeps ↑ and ↓** (#5993): React Aria's row hears them on their
 * way down and takes them from whatever is in it (a menu's button, so the arrows go on to the row
 * above or below), where a select's button and a combobox open their list with them. So the row
 * lets them by (`List.tsx`'s `rowElement`), and the control's own handler hears them and opens.
 */
export function keepsItsArrows(event: KeyboardEvent<HTMLElement>): boolean {
  if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return false;
  const control = event.target;
  if (!(control instanceof HTMLElement) || rowOf(control) === null) return false;
  return (
    control.getAttribute("aria-haspopup") === "listbox" ||
    control.getAttribute("role") === "combobox"
  );
}

/**
 * **A radio group in a row keeps its arrows** (#3123): the row hears them first, on their way
 * down, where React Aria's row would walk its controls with ← and → and leave it with ↑ and ↓,
 * and the radio group's own handler, under it, never hears them. So the list moves the choice
 * as a radio group does (the WAI-ARIA radio group): → and ↓ to the next radio, ← and ↑ to the
 * one before, round at the ends, ← and → turned round right to left; each choosing as it goes.
 * Answers whether it moved.
 */
export function movedInARadioGroup(event: KeyboardEvent<HTMLElement>, rtl: boolean): boolean {
  const radio = event.target;
  if (!(radio instanceof HTMLInputElement) || radio.type !== "radio") return false;
  if (rowOf(radio) === null || event.altKey || event.ctrlKey || event.metaKey) return false;
  const group = radio.closest('[role="radiogroup"]');
  if (group === null) return false;
  const ahead =
    event.key === "ArrowDown" ||
    (event.key === "ArrowRight" && !rtl) ||
    (event.key === "ArrowLeft" && rtl);
  const back =
    event.key === "ArrowUp" ||
    (event.key === "ArrowLeft" && !rtl) ||
    (event.key === "ArrowRight" && rtl);
  if (!ahead && !back) return false;
  const radios = [
    ...group.querySelectorAll<HTMLInputElement>('input[type="radio"]:not(:disabled)'),
  ];
  const at = radios.indexOf(radio);
  if (at === -1) return false;
  const to = radios[(at + (ahead ? 1 : radios.length - 1)) % radios.length];
  event.preventDefault();
  event.stopPropagation();
  to.focus();
  to.click();
  return true;
}
