// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What every pressable family shares on React Aria's `Button`** (#2904, #2913): the button family
 * and the status item family are each their own look over this one press, so a fix to it reaches
 * both.
 *
 * - **The native attributes React Aria does not hand on** ({@link NATIVE_KEYS}): a tooltip (until
 *   the shared tooltip replaces `title`, #2897), a busy press (#1190), a shortcut, and the place
 *   in a roving group's tab sequence. {@link pressable} splits them off.
 * - **A key the family's handler does not stop goes on** to its row, its list and the window, as
 *   a native button's does: React Aria stops every key at a handler unless told otherwise.
 * - **The element** ({@link renderPressed}): one that is unavailable but keeps the focus
 *   (`aria-disabled`, #1700) is drawn and themed as disabled, never as under the pointer or
 *   pressed, and every one is in WebKit's tab sequence (#190).
 */
import type { ButtonHTMLAttributes, ComponentPropsWithRef } from "react";
import type { KeyboardEvent } from "react-aria-components";

/** The native attributes a pressable passes to its element as they are. */
export type Native = Pick<
  ButtonHTMLAttributes<HTMLButtonElement>,
  | "title"
  | "tabIndex"
  | "aria-busy"
  | "aria-keyshortcuts"
  | "aria-live"
  | "aria-atomic"
  | "aria-owns"
  | "aria-roledescription"
  | "aria-errormessage"
>;

export const NATIVE_KEYS = [
  "title",
  "tabIndex",
  "aria-busy",
  "aria-keyshortcuts",
  "aria-live",
  "aria-atomic",
  "aria-owns",
  "aria-roledescription",
  "aria-errormessage",
] as const satisfies readonly (keyof Native)[];

/**
 * A call's props, split into what React Aria takes (its key handlers letting what they do not
 * stop go on) and the native attributes it would drop.
 */
export function pressable(props: Record<string, unknown>) {
  const native: Record<string, unknown> = {};
  const aria: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(props)) {
    if ((NATIVE_KEYS as readonly string[]).includes(key)) native[key] = value;
    else aria[key] = value;
  }
  for (const name of ["onKeyDown", "onKeyUp"] as const) {
    const handler = aria[name] as ((event: KeyboardEvent) => void) | undefined;
    if (handler)
      aria[name] = (event: KeyboardEvent) => {
        event.continuePropagation();
        handler(event);
      };
  }
  return { native, aria };
}

/** React Aria's `render` for a pressable: its `<button>`, with `native` handed on. */
export function renderPressed(native: Record<string, unknown>) {
  return (domProps: ComponentPropsWithRef<"button">) => {
    const unavailable = String(domProps["aria-disabled"]) === "true";
    const {
      "data-hovered": hovered,
      "data-pressed": pressed,
      ...rest
    } = domProps as ComponentPropsWithRef<"button"> & Record<`data-${string}`, unknown>;
    return (
      <button
        {...rest}
        data-hovered={unavailable ? undefined : (hovered as string | undefined)}
        data-pressed={unavailable ? undefined : (pressed as string | undefined)}
        data-disabled={unavailable ? "true" : (rest["data-disabled"] as string | undefined)}
        tabIndex={domProps.tabIndex ?? 0}
        {...native}
      />
    );
  };
}
