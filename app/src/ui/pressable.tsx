// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What every pressable family shares on React Aria's `Button`** (#2904, #2913, #2908): the
 * button family and the status item family are each their own look over this one press, so a
 * fix to it reaches both.
 *
 * - **The native attributes React Aria does not hand on** ({@link NATIVE_KEYS}): a busy press
 *   (#1190), a shortcut, and the place in a roving group's tab sequence. {@link pressable} splits
 *   them off. A hint is never a native `title`: it is the family's `tooltip` (#2908).
 * - **A key the family's handler does not stop goes on** to its row, its list and the window, as
 *   a native button's does: React Aria stops every key at a handler unless told otherwise.
 * - **Its tooltip** ({@link usePressTip}): what the shared tooltip shows, and how a screen reader
 *   hears it. **A disabled one with a hint keeps the focus** and does nothing
 *   (`ui/unavailable.ts`), described by its reason for good.
 * - **The element** ({@link renderPressed}): one that is unavailable but keeps the focus
 *   (`aria-disabled`, #1700) is drawn and themed as disabled, never as under the pointer or
 *   pressed, and every one is in WebKit's tab sequence (#190).
 */
import type { ButtonHTMLAttributes, ComponentPropsWithRef, ReactNode } from "react";
import type { KeyboardEvent } from "react-aria-components";
import { useId } from "react-aria";
import { says, TooltipDetail, TooltipSurface, type Tip } from "./tooltip/Tooltip";
import { unavailable } from "./unavailable";

/** The native attributes a pressable passes to its element as they are. */
export type Native = Pick<
  ButtonHTMLAttributes<HTMLButtonElement>,
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
  "tabIndex",
  "aria-busy",
  "aria-keyshortcuts",
  "aria-live",
  "aria-atomic",
  "aria-owns",
  "aria-roledescription",
  "aria-errormessage",
] as const satisfies readonly (keyof Native)[];

/** A pressable's tooltip, from {@link usePressTip}. */
export interface PressTip {
  /** The tooltip's surface, or `null` for no hint. */
  surface: ReactNode;
  /** It is disabled with a hint: it keeps the focus and does nothing. */
  keepsFocus: boolean;
  /** The hidden node a disabled one is described by, drawn beside it. */
  reason: ReactNode;
  /** `undefined`: the tooltip's own, as React Aria says it while it is shown; an id: that
   *  node's, while it is shown; `null`: nothing. */
  describedBy: string | null | undefined;
  /** The reason's id, which describes it always. */
  reasonId: string | null;
}

/**
 * **What the tooltip shows, and how a screen reader hears it.** A hint that is only the
 * element's name (an icon button's, or one that says its `aria-label` again) is shown and
 * describes nothing, so it is not heard twice. An icon button's second line is its description.
 * **A disabled one's reason is its description for good** (`reason`), whether or not its
 * tooltip is shown, so a screen reader that reaches it hears why.
 */
export function usePressTip(
  name: string | undefined,
  hint: Tip,
  iconOnly: boolean,
  disabled: boolean,
): PressTip {
  const detailId = useId();
  const reasonId = useId();
  let surface: ReactNode = null;
  let line: Tip;
  let describedBy: string | null | undefined = null;
  if (iconOnly && name !== undefined && name !== "") {
    const detail = says(hint) && hint !== name ? hint : undefined;
    line = detail;
    surface = (
      <TooltipSurface>
        {name}
        {detail !== undefined ? <TooltipDetail id={detailId}>{detail}</TooltipDetail> : null}
      </TooltipSurface>
    );
    describedBy = detail !== undefined ? detailId : null;
  } else if (says(hint)) {
    const named = hint === name;
    line = named ? undefined : hint;
    surface = <TooltipSurface>{hint}</TooltipSurface>;
    describedBy = named ? null : undefined;
  }
  const keepsFocus = disabled && surface !== null;
  const reason = keepsFocus && says(line) ? line : undefined;
  return {
    surface,
    keepsFocus,
    reason:
      reason === undefined ? null : (
        <span id={reasonId} hidden>
          {reason}
        </span>
      ),
    describedBy: reason === undefined ? describedBy : null,
    reasonId: reason === undefined ? null : reasonId,
  };
}

/**
 * A call's props, split into what React Aria takes (its key handlers letting what they do not
 * stop go on) and the native attributes it would drop. With a `tip` that keeps the focus, every
 * handler but the focus's and the pointer's is dropped (`ui/unavailable.ts`): React Aria's own
 * click is refused as the element is drawn ({@link renderPressed}), and its press stops Enter
 * and Space at the element already. `own` is the call's own description.
 */
export function pressable(props: Record<string, unknown>, tip?: PressTip) {
  const native: Record<string, unknown> = {};
  let aria: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(props)) {
    if ((NATIVE_KEYS as readonly string[]).includes(key)) native[key] = value;
    else aria[key] = value;
  }
  if (tip?.keepsFocus)
    aria = Object.fromEntries(
      Object.entries(unavailable(aria)).filter(
        ([key]) => !["isDisabled", "disabled", "onClick", "onKeyDown"].includes(key),
      ),
    );
  for (const name of ["onKeyDown", "onKeyUp"] as const) {
    const handler = aria[name] as ((event: KeyboardEvent) => void) | undefined;
    if (handler)
      aria[name] = (event: KeyboardEvent) => {
        event.continuePropagation();
        handler(event);
      };
  }
  return { native, aria, own: aria["aria-describedby"] as string | undefined };
}

/** React Aria's `render` for a pressable: its `<button>`, with `native` handed on. */
export function renderPressed(native: Record<string, unknown>, tip?: PressTip, own?: string) {
  return (domProps: ComponentPropsWithRef<"button">) => {
    const off = String(domProps["aria-disabled"]) === "true";
    const {
      "data-hovered": hovered,
      "data-pressed": pressed,
      ...rest
    } = domProps as ComponentPropsWithRef<"button"> & Record<`data-${string}`, unknown>;
    // While its tooltip is shown React Aria adds it to the description; a tooltip that is the
    // element's name is not, an icon button is described by its second line alone, and a
    // disabled one by its reason, always.
    let describedBy = rest["aria-describedby"];
    if (tip !== undefined) {
      const opened = rest["aria-describedby"] !== own;
      describedBy =
        tip.reasonId !== null
          ? [own, tip.reasonId].filter(Boolean).join(" ")
          : tip.describedBy === undefined
            ? rest["aria-describedby"]
            : [own, opened ? tip.describedBy : null].filter(Boolean).join(" ") || undefined;
    }
    // **It starts nothing** (#3116): a menu's or a popover's trigger around it hands React Aria
    // its own press and keys (`PressResponder`), which reach the element as handlers the call
    // never wrote. So the element itself keeps only the focus's and the pointer's (the tooltip's),
    // as a raw unavailable button does (`ui/unavailable.ts`), and opens nothing.
    const element = tip?.keepsFocus ? unavailable(rest) : rest;
    return (
      <button
        {...element}
        aria-describedby={describedBy}
        data-hovered={off ? undefined : (hovered as string | undefined)}
        data-pressed={off ? undefined : (pressed as string | undefined)}
        data-disabled={off ? "true" : (rest["data-disabled"] as string | undefined)}
        tabIndex={domProps.tabIndex ?? 0}
        {...native}
      />
    );
  };
}
