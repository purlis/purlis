// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Tab and Shift+Tab inside a dialog of the family** (#2910): every stop the keyboard can reach
 * in it, in reading order, round from the last to the first, moved to by the dialog itself.
 *
 * - **Moved by the dialog, not the engine.** WebKit leaves a button out of its own sequence
 *   unless its `tabindex` is written down (#186, `docs/ui-primitives.md`); a dialog that moves
 *   the keyboard itself reaches every control in every engine.
 * - **A group the arrows move inside is one stop** (a radio group, a tab list, a toolbar): its
 *   own element is a stop only while none of its items is, so Shift+Tab off its item leaves it
 *   rather than landing on it and being sent back to the item, which is what a library's roving
 *   group does when its element is focused.
 * - **What is not drawn is not a stop**: a closed `<details>` but for its `<summary>`, anything
 *   hidden, inert, disabled or out of the tab order.
 * - **A native radio group is one stop**, its checked radio, or its first while none is.
 */

/** What can take the keyboard, before what is out of the order is left out. */
const CANDIDATES =
  'a[href], area[href], button, input, select, textarea, summary, iframe, [contenteditable="true"], [tabindex]';

/** The roles of a group the arrows move inside: one stop for Tab. */
const GROUPS = new Set([
  "radiogroup",
  "tablist",
  "listbox",
  "menu",
  "menubar",
  "tree",
  "grid",
  "toolbar",
]);

/** The tab order an element says it is in: its `tabindex`, or 0 for what is reachable anyway. */
function order(element: HTMLElement): number {
  const written = element.getAttribute("tabindex");
  if (written !== null) return Number.parseInt(written, 10);
  if (element instanceof HTMLInputElement && element.type === "hidden") return -1;
  if (element.matches("a[href], area[href], button, input, select, textarea, iframe")) return 0;
  if (element.localName === "summary") return 0;
  if (element.isContentEditable || element.getAttribute("contenteditable") === "true") return 0;
  return -1;
}

/** Whether `element` is drawn, up to `within`: not hidden, not inert, not in a closed details. */
function drawn(element: HTMLElement, within: HTMLElement): boolean {
  for (let at: HTMLElement | null = element; at !== null; at = at.parentElement) {
    if (at.hidden || at.inert) return false;
    const style = getComputedStyle(at);
    if (style.display === "none" || style.visibility === "hidden") return false;
    const details: HTMLElement | null = at.parentElement;
    if (
      details instanceof HTMLDetailsElement &&
      !details.open &&
      !(at.localName === "summary" && details.querySelector(":scope > summary") === at)
    )
      return false;
    if (at === within) break;
  }
  return true;
}

/** Whether `element` is a group's own element while one of its items is a stop. */
function aGroupWithAStop(element: HTMLElement): boolean {
  const role = element.getAttribute("role");
  return role !== null && GROUPS.has(role) && element.querySelector('[tabindex="0"]') !== null;
}

/** Whether a native radio is its group's one stop. */
function theRadiosStop(radio: HTMLInputElement, within: HTMLElement): boolean {
  if (radio.name === "") return true;
  const group = [...within.querySelectorAll<HTMLInputElement>("input[type='radio']")].filter(
    (one) => one.name === radio.name && !one.disabled,
  );
  const checked = group.find((one) => one.checked);
  return checked !== undefined ? checked === radio : group[0] === radio;
}

/**
 * Where the keyboard lands on `stop`: on a group's own element, its chosen item, else its first,
 * as a library's roving group would send it a moment later; anywhere else, the stop itself.
 */
export function entryOf(stop: HTMLElement): HTMLElement {
  const role = stop.getAttribute("role");
  if (role === null || !GROUPS.has(role)) return stop;
  return (
    stop.querySelector<HTMLElement>(
      '[aria-checked="true"], [aria-selected="true"], [data-state="checked"]',
    ) ??
    stop.querySelector<HTMLElement>(
      '[role="radio"], [role="tab"], [role="option"], [role="menuitem"], [role="treeitem"]',
    ) ??
    stop
  );
}

/** Every stop of `dialog`, in reading order. */
export function stopsIn(dialog: HTMLElement): HTMLElement[] {
  return [...dialog.querySelectorAll<HTMLElement>(CANDIDATES)].filter((element) => {
    if (order(element) < 0) return false;
    if (element.matches(":disabled")) return false;
    if (!drawn(element, dialog)) return false;
    if (aGroupWithAStop(element)) return false;
    if (element instanceof HTMLInputElement && element.type === "radio")
      return theRadiosStop(element, dialog);
    return true;
  });
}

/**
 * The stop Tab (or, with `back`, Shift+Tab) goes to from `from` in `dialog`: the next in reading
 * order, round from the last to the first. From somewhere that is not a stop (the dialog itself,
 * a row that took the keyboard), the next stop after it in the document.
 */
export function nextStop(dialog: HTMLElement, from: Element, back: boolean): HTMLElement | null {
  const stops = stopsIn(dialog);
  if (stops.length === 0) return null;
  const at = stops.indexOf(from as HTMLElement);
  if (at >= 0) return entryOf(stops[(at + (back ? -1 : 1) + stops.length) % stops.length]);
  // From inside a group (an item that is not its stop yet), past the group.
  const group = stops.find((stop) => stop !== from && stop.contains(from));
  const after = group ?? from;
  const side = back ? Node.DOCUMENT_POSITION_PRECEDING : Node.DOCUMENT_POSITION_FOLLOWING;
  const beyond = stops.filter(
    (stop) => !stop.contains(after) && after.compareDocumentPosition(stop) & side,
  );
  if (beyond.length === 0) return entryOf(back ? stops[stops.length - 1] : stops[0]);
  return entryOf(back ? beyond[beyond.length - 1] : beyond[0]);
}
