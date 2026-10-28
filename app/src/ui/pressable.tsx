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
 *   (`aria-disabled`, #1700, given by hand or by `isDisabled` with a hint) is drawn and themed as
 *   disabled, never as under the pointer or pressed, and runs no press (#3522); every one is in
 *   WebKit's tab sequence (#190).
 */
import type { ButtonHTMLAttributes, ComponentPropsWithRef, ReactNode } from "react";
import type { KeyboardEvent } from "react-aria-components";
import { useId } from "react-aria";
import { says, TooltipDetail, TooltipSurface, type Tip } from "./tooltip/Tooltip";
import { refused, unavailable } from "./unavailable";

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
 * stop go on) and the native attributes it would drop. `own` is the call's own description.
 *
 * With a `tip` that keeps the focus, it is not disabled to React Aria (which would draw the
 * native `disabled`, out of the tab sequence) but said unavailable (`aria-disabled`). Every
 * handler but the focus's and the pointer's is dropped as the element is drawn
 * ({@link renderPressed}), the call's own and a trigger's alike; here the call's are dropped too,
 * so React Aria is handed no press to hold.
 */
export function pressable(props: Record<string, unknown>, tip?: PressTip) {
  const native: Record<string, unknown> = {};
  let aria: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(props)) {
    if ((NATIVE_KEYS as readonly string[]).includes(key)) native[key] = value;
    else aria[key] = value;
  }
  if (tip?.keepsFocus)
    aria = {
      ...Object.fromEntries(
        Object.entries(aria).filter(
          ([key]) => !refused(key) && key !== "isDisabled" && key !== "disabled",
        ),
      ),
      "aria-disabled": true,
    };
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

/**
 * **Whether a call's own `aria-expanded` is a disclosure's** (#3524): it says its region is out
 * and opens no menu or popover (`aria-haspopup`), so it is not drawn open. A trigger's, whether
 * a menu's or a popover's hands it to the element (`PressResponder`) or the call writes it with
 * its `aria-haspopup`, is.
 */
export function discloses(props: Record<string, unknown>): boolean {
  const popup = props["aria-haspopup"];
  return (
    props["aria-expanded"] !== undefined &&
    (popup === undefined || popup === false || popup === "false")
  );
}

/**
 * React Aria's `render` for a pressable: its `<button>`, with `native` handed on. `disclosure`:
 * the call's own `aria-expanded` is a disclosure's ({@link discloses}), never drawn open.
 */
export function renderPressed(
  native: Record<string, unknown>,
  tip?: PressTip,
  own?: string,
  disclosure = false,
) {
  return (domProps: ComponentPropsWithRef<"button">) => {
    const off = String(domProps["aria-disabled"]) === "true";
    // **Open** (#3524): the menu or the popover it is the trigger of is open, which React Aria
    // says only as `aria-expanded`; the family draws it from `data-open`, as a theme's
    // `<family>.root.open` reaches it. Never while it is unavailable.
    const open = !off && !disclosure && String(domProps["aria-expanded"]) === "true";
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
    // **It starts nothing** (#3116, #3522), whether it was made unavailable by `isDisabled` with
    // a hint or by hand with `aria-disabled`: React Aria's press, and a menu's or a popover's
    // trigger around it (`PressResponder`), reach the element as handlers the call never wrote.
    // So the element itself keeps only the focus's and the pointer's (the tooltip's), as a raw
    // unavailable button does (`ui/unavailable.ts`): it runs no press, submits no form and opens
    // nothing.
    const element = off ? unavailable(rest) : rest;
    return (
      <button
        {...element}
        aria-describedby={describedBy}
        data-hovered={off ? undefined : (hovered as string | undefined)}
        data-pressed={off ? undefined : (pressed as string | undefined)}
        data-disabled={off ? "true" : (rest["data-disabled"] as string | undefined)}
        data-open={open ? "" : undefined}
        tabIndex={domProps.tabIndex ?? 0}
        {...native}
      />
    );
  };
}
