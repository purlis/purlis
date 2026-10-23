// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a target's tooltip says, as the keyboard reads it** (#2908): the way a test reads a
 * hint where it used to read a native `title`. The keyboard's focus opens a tooltip at once
 * (`Tooltip.tsx`), so this is the person's own way to it, without waiting out the pointer's
 * delay: the target is focused as the keyboard would focus it, the tooltip shown is read, and
 * the target is let go again, which takes the tooltip down.
 *
 * A mark that takes no keyboard (one inside a row, or hidden from a screen reader) is the
 * pointer's: it is lent a place to hold the focus for the read, so what it says can still be
 * read here. Whether a target is reached by the keyboard is `Tooltip.test.tsx`'s to hold.
 *
 * `null` when the target shows no tooltip.
 */
import { act, fireEvent } from "@testing-library/react";
import { TOOLTIP_CLOSE_DELAY, TOOLTIP_DELAY } from "./Tooltip";

/** The tooltip shown now, if one is. */
export function shownTip(): HTMLElement | null {
  return document.querySelector<HTMLElement>('[role="tooltip"]:not([data-exiting])');
}

/** The tooltip `target` opened: the one it is described by, else the one shown (an icon
 *  button's, which says its name and describes nothing). */
function tipDescribing(target: HTMLElement | SVGElement): HTMLElement | null {
  for (const id of (target.getAttribute("aria-describedby") ?? "").split(/\s+/)) {
    const one = id === "" ? null : document.getElementById(id);
    if (one?.getAttribute("role") === "tooltip") return one;
  }
  // An icon button's tooltip says its name and describes nothing, and an element that says its
  // own description keeps it while its tooltip is shown.
  return target.matches(HINTED) ? shownTip() : null;
}

export function tipOf(target: Element | null | undefined): string | null {
  // An SVG drawing takes a tooltip too (a gauge's trend).
  if (!(target instanceof HTMLElement || target instanceof SVGElement)) return null;
  // The keyboard is what moves the focus now, as React Aria tells it apart from the pointer.
  // Focused already (a dialog's first control, by the pointer's press that opened it), it is let
  // go first, so the keyboard's focus is a new one.
  if (document.activeElement === target) act(() => target.blur());
  fireEvent.keyDown(document.body, { key: "Tab" });
  fireEvent.keyUp(document.body, { key: "Tab" });
  act(() => target.focus());
  // A mark that takes no keyboard is given a place to hold the focus for the read, and loses it
  // again.
  const lent = document.activeElement !== target && !target.hasAttribute("tabindex");
  if (lent) {
    target.tabIndex = -1;
    act(() => target.focus());
  }
  const said =
    document.activeElement === target ? (tipDescribing(target)?.textContent ?? null) : null;
  act(() => target.blur());
  if (lent) target.removeAttribute("tabindex");
  return said;
}

/** Every element a hint may be on: words and marks with one, the button family, and the status
 *  item family. */
const HINTED =
  '[data-tooltip-target], [data-scope="button"][data-part="root"], [data-scope="status-item"][data-part="root"]';

type Said = string | RegExp;

const matches = (said: string | null, wanted: Said) =>
  said !== null && (typeof wanted === "string" ? said.trim() === wanted : wanted.test(said));

/** Each element under `within` whose tooltip says `wanted`, as `getAllByTitle` found them. */
export function queryAllByTip(wanted: Said, within: Element = document.body): HTMLElement[] {
  return [...within.querySelectorAll<HTMLElement>(HINTED)].filter((one) =>
    matches(tipOf(one), wanted),
  );
}

export function queryByTip(wanted: Said, within?: Element): HTMLElement | null {
  const found = queryAllByTip(wanted, within);
  if (found.length > 1) throw new Error(`${found.length} elements have the tooltip ${wanted}`);
  return found[0] ?? null;
}

export function getAllByTip(wanted: Said, within?: Element): HTMLElement[] {
  const found = queryAllByTip(wanted, within);
  if (found.length === 0) throw new Error(`No element has the tooltip ${wanted}`);
  return found;
}

export function getByTip(wanted: Said, within?: Element): HTMLElement {
  const found = queryByTip(wanted, within);
  if (found === null) throw new Error(`No element has the tooltip ${wanted}`);
  return found;
}

/**
 * **`check` until it passes, or a second has gone**: `waitFor` for a check that reads a tooltip.
 * `waitFor` runs its check again on every change to the document, and reading a tooltip opens
 * and closes one, which is a change: the two would chase each other for ever. This waits on the
 * clock alone.
 */
export async function eventually<T>(check: () => T, timeout = 1000): Promise<T> {
  const started = Date.now();
  for (;;) {
    try {
      return check();
    } catch (error) {
      if (Date.now() - started > timeout) throw error;
    }
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
  }
}

/** `getByTip`, waited for as `findByTitle` waited. */
export function findByTip(wanted: Said, within?: Element): Promise<HTMLElement> {
  return eventually(() => getByTip(wanted, within));
}

export function findAllByTip(wanted: Said, within?: Element): Promise<HTMLElement[]> {
  return eventually(() => getAllByTip(wanted, within));
}

/**
 * **What a target's tooltip says, as the pointer reads it**: the way to a hint on a control the
 * keyboard cannot reach, a disabled one. The pointer moves over the window, rests on the target
 * past the tooltip's delay, and leaves.
 */
export async function hoverTipOf(target: Element | null | undefined): Promise<string | null> {
  if (!(target instanceof HTMLElement)) return null;
  const rest = (ms: number) =>
    act(async () => {
      await new Promise((resolve) => setTimeout(resolve, ms));
    });
  fireEvent.pointerMove(document.body, { pointerType: "mouse" });
  fireEvent.pointerOver(target, { pointerType: "mouse" });
  fireEvent.pointerEnter(target, { pointerType: "mouse" });
  await rest(TOOLTIP_DELAY + 50);
  const said = tipDescribing(target)?.textContent ?? shownTip()?.textContent ?? null;
  fireEvent.pointerLeave(target, { pointerType: "mouse" });
  fireEvent.pointerOut(target, { pointerType: "mouse" });
  await rest(TOOLTIP_CLOSE_DELAY + 50);
  return said;
}
