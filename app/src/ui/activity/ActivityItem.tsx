// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The activity family** (#2917, spec #2897 R116–R120, N17, ADR 0104): a view's icon on a side's
 * bar, and the bar that holds them, drawn from one wrapper each. A screen imports these and never
 * draws an activity item's look itself (`purlis/activity-family` in `eslint.config.js`).
 *
 * - **An item is drawn by its parts.** Its root carries `data-scope="activity"`,
 *   `data-part="item"` and its `data-variant`: `bar` (a side's 36px column, both corners' since
 *   #3144) or `tab` (32×30 tabs across a side panel's top, which no bar of the window draws now
 *   but a theme may still name). Its mark, its count and the mark along
 *   its edge while it is chosen are the `icon`, `count` and `indicator` parts. `activity.css`
 *   draws the defaults, and a theme's parts reach the same attributes (ADR 0103).
 * - **Its states are marked as React Aria marks a button's**: `data-hovered` under a pointer that
 *   hovers, `data-pressed` while pressed, `data-selected` while its view is open and
 *   `data-focus-visible` while the keyboard is on it. The item stays a native button, because it
 *   is a tab of its bar's roving group (`ActivityBar`), whose item hands it its keys and focus.
 * - **A state changes a colour, never a size**: an item is the same box chosen or not, so nothing
 *   beside it moves.
 */
import {
  useState,
  type ComponentPropsWithRef,
  type FocusEvent,
  type PointerEvent,
  type ReactNode,
} from "react";
import { Tooltip } from "../tooltip/Tooltip";

/** The family's variants, in the order the manifest names them. */
export const ACTIVITY_VARIANTS = ["tab", "bar"] as const;
export type ActivityVariant = (typeof ACTIVITY_VARIANTS)[number];

export interface ActivityItemProps extends Omit<
  ComponentPropsWithRef<"button">,
  "style" | "className" | "children"
> {
  variant: ActivityVariant;
  /** Its view is open: the `selected` state. */
  selected: boolean;
  /** Its view's mark: the `icon` part. Decoration; the item's name is its `aria-label`. */
  icon: ReactNode;
  /** What is drawn at its top right: an {@link ActivityCount}, or nothing. */
  count?: ReactNode;
  /** **What its tooltip says under its name** (`ui/tooltip`): what a press does. An icon-only
   *  button's tooltip is its name first. */
  tip?: string;
  /** `data-*` attributes reach the element as they are. */
  [data: `data-${string}`]: string | number | boolean | undefined;
}

/**
 * **One activity item**: a native `<button>` with the family's attributes and states. Every prop
 * a roving group's item hands its child (its focus and key handlers, its `tabIndex`, its `ref`)
 * reaches the button, each state handler run after the one handed in.
 */
export function ActivityItem({
  variant,
  selected,
  icon,
  count,
  tip,
  onPointerEnter,
  onPointerLeave,
  onPointerDown,
  onPointerUp,
  onPointerCancel,
  onFocus,
  onBlur,
  ...rest
}: ActivityItemProps) {
  const [hovered, setHovered] = useState(false);
  const [pressed, setPressed] = useState(false);
  const [keyboard, setKeyboard] = useState(false);
  // A touch has no pointer to hover with, as React Aria has it: only a mouse or a pen hovers.
  const hovers = (e: PointerEvent) => e.pointerType !== "touch";
  const item = (
    <button
      type="button"
      {...rest}
      data-scope="activity"
      data-part="item"
      data-variant={variant}
      data-selected={selected || undefined}
      data-hovered={hovered || undefined}
      data-pressed={pressed || undefined}
      data-focus-visible={keyboard || undefined}
      onPointerEnter={(e) => {
        onPointerEnter?.(e);
        if (hovers(e)) setHovered(true);
      }}
      onPointerLeave={(e) => {
        onPointerLeave?.(e);
        setHovered(false);
        setPressed(false);
      }}
      onPointerDown={(e) => {
        onPointerDown?.(e);
        setPressed(true);
      }}
      onPointerUp={(e) => {
        onPointerUp?.(e);
        setPressed(false);
      }}
      onPointerCancel={(e) => {
        onPointerCancel?.(e);
        setPressed(false);
      }}
      onFocus={(e: FocusEvent<HTMLButtonElement>) => {
        onFocus?.(e);
        setKeyboard(focusVisible(e.currentTarget));
      }}
      onBlur={(e: FocusEvent<HTMLButtonElement>) => {
        onBlur?.(e);
        setKeyboard(false);
      }}
    >
      <span data-scope="activity" data-part="icon" aria-hidden="true">
        {icon}
      </span>
      {count}
      <span data-scope="activity" data-part="indicator" aria-hidden="true" />
    </button>
  );
  return tip === undefined ? item : <Tooltip tip={tip}>{item}</Tooltip>;
}

/** Whether the browser draws `element`'s focus as the keyboard's (`:focus-visible`). An engine
 *  that does not know the selector draws it as any focus. */
function focusVisible(element: Element): boolean {
  try {
    return element.matches(":focus-visible");
  } catch {
    return element.matches(":focus");
  }
}

/**
 * **A count on an item** (B-8, R119): the number drawn at the item's top right, and the sentence a
 * screen reader is told as the item's description, since the item's name is its view's. What waits
 * on the person is drawn hot (`needs-you`, R120); every other count is plain. Nothing is drawn for
 * nothing: a zero is not a count.
 */
export function ActivityCount({
  count,
  said,
  tone,
  drawn,
}: {
  count: number;
  said: string;
  tone?: "needs-you";
  /** What is drawn in place of the bare number, where one number does not say it: Repos'
   *  commits behind and ahead (#1739). Still nothing at all for a zero `count`. */
  drawn?: string;
}) {
  if (count <= 0) return null;
  return (
    // The number is drawn by the stylesheet from `data-count`: no text of the item's own, so its
    // name stays its view's alone (axe's label and content match, WCAG 2.5.3).
    <span data-scope="activity" data-part="count" data-tone={tone} data-count={drawn ?? count}>
      <span data-scope="activity" data-part="said">
        {said}
      </span>
    </span>
  );
}

/**
 * **A group of a bar's items**: one tab list, the side panel's views or its half of the bottom
 * panel's. Every prop reaches the element (a roving group's root hands it its own), and the
 * family's attributes are its own.
 */
export function ActivityGroup({
  children,
  ...rest
}: Omit<ComponentPropsWithRef<"div">, "style"> & {
  [data: `data-${string}`]: string | number | boolean | undefined;
}) {
  return (
    <div {...rest} data-scope="activity-bar" data-part="group">
      {children}
    </div>
  );
}

/**
 * **A side's bar**: its items in their order, the top group, the gap, then the bottom group. A
 * `bar` is a column (the left one beside the project rail, the right one at the window's edge,
 * #3144); a `tab` row is drawn across a side panel's top over its rule.
 */
export function ActivityBarFrame({
  variant,
  top,
  bottom,
  ...rest
}: Omit<ComponentPropsWithRef<"div">, "style" | "children"> & {
  variant: ActivityVariant;
  /** The side panel's views' tab list. */
  top: ReactNode;
  /** Its half of the bottom panel's, after the gap. */
  bottom: ReactNode;
  [data: `data-${string}`]: string | number | boolean | undefined;
}) {
  return (
    <div {...rest} data-scope="activity-bar" data-part="root" data-variant={variant}>
      <div data-scope="activity-bar" data-part="items">
        {top}
        <span data-scope="activity-bar" data-part="gap" aria-hidden="true" />
        {bottom}
      </div>
      {variant === "tab" && (
        <div data-scope="activity-bar" data-part="separator" aria-hidden="true" />
      )}
    </div>
  );
}
