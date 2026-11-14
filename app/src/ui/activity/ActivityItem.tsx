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
 *   is a tab of its bar's strip (`ActivityBar`, `AsStripTab`), which hands it its role and focus.
 * - **A state changes a colour, never a size**: an item is the same box chosen or not, so nothing
 *   beside it moves.
 */
import {
  useRef,
  useState,
  type ComponentPropsWithRef,
  type FocusEvent,
  type KeyboardEvent,
  type PointerEvent,
  type ReactNode,
} from "react";
import { mergeProps, useToolbar } from "react-aria";
import { RoundCount } from "../badge/Badge";
import { Tooltip } from "../tooltip/Tooltip";
import { useWindowLeft } from "../windowLeft";

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
  /** **What it opens is out of date** (#5955): an artifact whose data is stale. Its icon is
   *  drawn in the faint grey; a chosen item keeps the chosen look. Say why in its `tip`. */
  dimmed?: boolean;
  /** Its view's mark: the `icon` part. Decoration; the item's name is its `aria-label`. */
  icon: ReactNode;
  /** What is drawn at its top right: an {@link ActivityCount}, or nothing. */
  count?: ReactNode;
  /** **What its tooltip says under its name** (`ui/tooltip`): what a press does. An icon-only
   *  button's tooltip is its name first. */
  tip?: string;
  /** The key that does what a press does, after the tooltip's words (#2921). */
  shortcut?: string;
  /** `data-*` attributes reach the element as they are. */
  [data: `data-${string}`]: string | number | boolean | undefined;
}

/**
 * **One activity item**: a native `<button>` with the family's attributes and states. Every prop
 * a strip's tab (`AsStripTab`) hands it (its focus and key handlers, its `tabIndex`, its `ref`)
 * reaches the button, each state handler run after the one handed in.
 */
export function ActivityItem({
  variant,
  selected,
  dimmed,
  icon,
  count,
  tip,
  shortcut,
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
  // Gone from the window, which an engine may not say on the item (#4628).
  useWindowLeft((why) => {
    if (why !== "pointer") return;
    setHovered(false);
    setPressed(false);
  }, hovered || pressed);
  const item = (
    <button
      type="button"
      {...rest}
      data-scope="activity"
      data-part="item"
      data-variant={variant}
      data-selected={selected || undefined}
      data-dimmed={dimmed || undefined}
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
  return tip === undefined ? (
    item
  ) : (
    <Tooltip tip={tip} shortcut={shortcut}>
      {item}
    </Tooltip>
  );
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
  // What needs the person is the badge family's round count (#6342): one character, centred on
  // the icon's top right corner; this part places it, keeps the sentence and the real number.
  if (tone === "needs-you")
    return (
      <span data-scope="activity" data-part="count" data-tone={tone} data-count={count}>
        <RoundCount count={count} at="mark" />
        <span data-scope="activity" data-part="said">
          {said}
        </span>
      </span>
    );
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
 * **A dot on an item** (ADR 0119, #5645; ADR 0121 decision 3, #6004): where the count is drawn, a
 * mark and never a number: in the danger colour (`danger`) for something that needs fixing, in
 * the waiting amber (`warning`) for a decision that stops no chat. `said` is the item's
 * description, as a count's sentence is, so the colour is never the only thing that says it. Each
 * tone is its own role, as the needs-you count's is: a count rule of `parts` never reaches it.
 */
export function ActivityDot({
  said,
  tone = "danger",
}: {
  said: string;
  tone?: "danger" | "warning";
}) {
  return (
    <span data-scope="activity" data-part="count" data-tone={tone}>
      <span data-scope="activity" data-part="said">
        {said}
      </span>
    </span>
  );
}

/**
 * **A "new" pip on an item** (#6002, ADR 0121 decision 10): a dot in the accent colour at the
 * item's bottom right, while its view lists something the person has not seen. It stands apart
 * from a count or a dot at the top right, so both are drawn together. `said` is told with the
 * item's description, as a count's sentence is.
 */
export function ActivityNew({ said }: { said: string }) {
  return (
    <span data-scope="activity" data-part="pip">
      <span data-scope="activity" data-part="said">
        {said}
      </span>
    </span>
  );
}

/**
 * **A group of a bar's items**: one tab list, the side panel's views or its half of the bottom
 * panel's. Every prop reaches the element (a strip's row hands it its keyboard and its
 * tablist's role, `useTabStripRow`), and the family's attributes are its own.
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
 * **A group of a bar's items that open tabs, not views** (#5952): a toolbar of buttons,
 * React Aria's (`useToolbar`). The arrows walk it along `orientation`, Home and End reach its
 * first and last item, and Tab leaves it, so it is one Tab stop.
 *
 * **Coming back in lands on the item in front**, the one marked `aria-current`, from either
 * side, as a tab list lands on its selected tab; with none in front, on the item it left. A
 * click is not coming back in: the focus stays on the item clicked (#6120).
 *
 * **An item being carried keeps its keys**: while an item is marked `data-dragging`, its
 * arrows are its drag's (`sortable.tsx`, `keepsTheFocus`), so the toolbar leaves them alone.
 */
export function ActivityToolbar({
  children,
  orientation,
  "aria-label": label,
  ...rest
}: Omit<ComponentPropsWithRef<"div">, "style" | "role" | "aria-orientation" | "ref"> & {
  orientation: "vertical" | "horizontal";
  "aria-label": string;
  [data: `data-${string}`]: string | number | boolean | undefined;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const { toolbarProps } = useToolbar({ orientation, "aria-label": label }, ref);
  const walk = toolbarProps.onKeyDownCapture;
  const restore = toolbarProps.onFocusCapture;
  const own = {
    ...toolbarProps,
    onPointerDownCapture: (event: PointerEvent<HTMLDivElement>) => {
      pressing.add(event.currentTarget);
    },
    onPointerUpCapture: (event: PointerEvent<HTMLDivElement>) => {
      pressing.delete(event.currentTarget);
    },
    onPointerCancelCapture: (event: PointerEvent<HTMLDivElement>) => {
      pressing.delete(event.currentTarget);
    },
    onKeyDownCapture: (event: KeyboardEvent<HTMLDivElement>) => {
      const target = event.target as HTMLElement;
      if (target.closest("[data-dragging]") !== null) return;
      if (event.key === "Home" || event.key === "End") {
        const all = itemsOf(event.currentTarget);
        const to = event.key === "Home" ? all[0] : all[all.length - 1];
        if (to === undefined) return;
        event.preventDefault();
        event.stopPropagation();
        to.focus();
        return;
      }
      walk?.(event);
    },
    onFocusCapture: (event: FocusEvent<HTMLDivElement>) => {
      const pressed = pressing.delete(event.currentTarget);
      const from = event.relatedTarget;
      const entering = !(from instanceof Node && event.currentTarget.contains(from));
      const inFront = itemsOf(event.currentTarget).find((item) =>
        item.hasAttribute("aria-current"),
      );
      if (!entering || inFront === undefined || pressed) {
        restore?.(event);
        return;
      }
      if (inFront !== event.target) inFront.focus();
    },
  };
  return (
    <div {...mergeProps(rest, own)} ref={ref} data-scope="activity-bar" data-part="group">
      {children}
    </div>
  );
}

/**
 * The toolbars a press is in, until the focus it brings (#6120): an engine that focuses a button
 * on its press, as Chromium and Firefox do and WebKit does not, brings the focus in from outside,
 * and that is a click on its item, never the keyboard coming back in.
 */
const pressing = new WeakSet<Element>();

/** The items of a toolbar that take the keyboard, in the document's order. */
function itemsOf(toolbar: HTMLElement): HTMLElement[] {
  return Array.from(
    toolbar.querySelectorAll<HTMLElement>(
      '[data-scope="activity"][data-part="item"]:not(:disabled)',
    ),
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
