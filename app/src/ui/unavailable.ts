// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A control that is unavailable and keeps the focus** (#2908): a disabled button whose hint
 * says why, the button family's and a raw `<button>`'s alike. It stays a Tab stop, so the reason
 * is read by the keyboard and a screen reader, and it does nothing at all.
 *
 * **An allowlist, not a denylist.** Of everything a call site and its parents hand the element
 * (a Radix trigger's `onClick`, a row's `onKeyDown`, a form's submit), only what follows the
 * focus and the pointer stays: focus, blur and hover, which the tooltip itself listens to.
 * Every other handler is dropped, and a click, Enter and Space on it are stopped there: nothing
 * around it (a row, a list) acts on them either. **It keeps its `type`** (#3636): a submit button
 * stays one, so its form still finds its primary action unavailable and ⌘⏎ sends nothing (N26),
 * and the click that would submit it (a pointer's, a screen reader's, the one Enter in a lone
 * field fires at it) is cancelled by {@link refuseClick}. One that says no type is a plain
 * `type="button"`.
 */

/** The handlers an unavailable control keeps: the focus's and the pointer's comings and goings. */
const KEPT = new Set([
  "onFocus",
  "onBlur",
  "onFocusChange",
  "onFocusIn",
  "onFocusOut",
  "onHoverStart",
  "onHoverEnd",
  "onHoverChange",
  "onPointerEnter",
  "onPointerLeave",
  "onPointerOver",
  "onPointerOut",
  "onPointerMove",
  "onMouseEnter",
  "onMouseLeave",
  "onMouseOver",
  "onMouseOut",
  "onMouseMove",
]);

const HANDLER = /^on[A-Z]/;

/** Whether `name` is a handler an unavailable control drops. */
export const refused = (name: string): boolean => HANDLER.test(name) && !KEPT.has(name);

type Stoppable = { preventDefault: () => void; stopPropagation: () => void };

/**
 * **A press on an unavailable control goes nowhere**: its click does nothing, a form's submit
 * included, and reaches no row or list around it, as a natively disabled button's click never
 * happens at all.
 */
export const refuseClick = (event: Stoppable) => {
  event.preventDefault();
  event.stopPropagation();
};

/** Enter and Space on it press nothing around it either; every other key goes on (Tab, Esc). */
export const refusePressKeys = (event: Stoppable & { key: string }) => {
  if (event.key === "Enter" || event.key === " ") refuseClick(event);
};

/**
 * `props` as an unavailable control that keeps the focus takes them: every handler but the
 * focus's and the pointer's dropped, its own `type` kept (a plain button when it says none), its
 * press refused, marked unavailable.
 */
export function unavailable<T extends Record<string, unknown>>(props: T): Record<string, unknown> {
  const kept: Record<string, unknown> = {};
  for (const [name, value] of Object.entries(props)) if (!refused(name)) kept[name] = value;
  return {
    ...kept,
    disabled: undefined,
    type: props.type ?? "button",
    onClick: refuseClick,
    onKeyDown: refusePressKeys,
    "aria-disabled": true,
  };
}
