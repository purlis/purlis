// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The status item family** (#2913, spec #2897 N28 and R115, ADR 0104): every item of the status
 * bar and every status pill at the title bar's right, drawn from one wrapper. A screen imports
 * these and never draws its own (`purlis/status-item-family` in `eslint.config.js`).
 *
 * - **Drawn by its parts.** The root carries `data-scope="status-item"`, `data-part="root"` and
 *   its `data-variant`: `bar` (a cell of the status bar), `pill` (a pill at the title bar's right:
 *   24px, full corners, 12px words), `waiting` (the pill that says what waits on the person) and
 *   `accent` (the pill that is news). Its icon, its words and its count are the `icon`, `label`
 *   and `count` parts. `status.css` draws the defaults, and a theme's parts reach the same
 *   attributes (ADR 0103).
 * - **What can be pressed runs a command**: {@link StatusItem}, on React Aria's `Button`, so it is
 *   pressed by pointer, Enter and Space and shows its ring from the keyboard only. What only says
 *   something is a {@link StatusCell}: its words, never a button.
 * - **A tone is a colour beside words, never instead of them** (qw357): `waiting`, `failed`,
 *   `success`, `quiet`, `accent` or `danger` colours the item's icon (and, for `danger`, the item),
 *   from the state tokens a theme sets.
 * - **Its hint is the shared tooltip** (#2908, `ui/tooltip`), never a native `title`: `tooltip`
 *   on either member. **A disabled item with a hint keeps the focus** and does nothing, as a
 *   disabled button with one does (`ui/pressable.tsx`): the hint says why.
 * - **24px tall in either density** (R115), which is also the least target (WCAG 2.2): a state
 *   changes a colour, never a size.
 */
import { Children, type ComponentPropsWithRef, type ReactNode, type Ref } from "react";
import { Button as AriaButton, type ButtonProps as AriaButtonProps } from "react-aria-components";
import { discloses, pressable, renderPressed, usePressTip, type Native } from "../pressable";
import { Tipped, Tooltip, type Tip } from "../tooltip/Tooltip";

/** The family's variants, in the order the manifest names them. */
export const STATUS_VARIANTS = ["bar", "pill", "waiting", "accent"] as const;
export type StatusVariant = (typeof STATUS_VARIANTS)[number];

/** The colour an item's icon takes beside its words. */
export const STATUS_TONES = ["waiting", "failed", "success", "quiet", "accent", "danger"] as const;
export type StatusTone = (typeof STATUS_TONES)[number];

/** What every member of the family takes. */
interface Drawn {
  variant?: StatusVariant;
  tone?: StatusTone;
  /** Its words in the strong weight: the name of where the person is. */
  strong?: boolean;
  /** It gives way first in a narrow bar, its words cut with an ellipsis: a long name. */
  shrinks?: boolean;
  /** An icon before the words: the `icon` part. Decoration (`aria-hidden`), or a mark that says
   *  something (`role="img"` with a name). */
  icon?: ReactNode;
  /** A count after the words: the `count` part. A pressable item's name says what it counts,
   *  so its count is not said twice; a cell's is read with its words. */
  count?: ReactNode;
  /** **Its hint**, the shared tooltip (#2908): what the item is, or why it is unavailable. */
  tooltip?: Tip;
  /** A place for the item (margin, flex, order), never a look. */
  className?: string;
  children?: ReactNode;
  /** `data-*` attributes reach the element as they are. */
  [data: `data-${string}`]: string | number | boolean | undefined;
}

export interface StatusItemProps
  extends Drawn, Native, Omit<AriaButtonProps, "className" | "style" | "children" | "render"> {
  ref?: Ref<HTMLButtonElement>;
}

/** The parts inside: the icon, each run of words as the label, and the count, which is said
 *  only where the item's name does not already say it (`spoken`). */
function parts(
  { icon, count, children }: Pick<Drawn, "icon" | "count" | "children">,
  spoken: boolean,
) {
  const out: ReactNode[] = [];
  if (icon !== undefined && icon !== null)
    out.push(
      // The icon is decoration its own `aria-hidden` says, or a labelled mark (`role="img"`).
      <span key="icon" data-scope="status-item" data-part="icon">
        {icon}
      </span>,
    );
  let run: (string | number)[] = [];
  const close = () => {
    if (run.length === 0) return;
    out.push(
      <span key={`label-${out.length}`} data-scope="status-item" data-part="label">
        {run.join("")}
      </span>,
    );
    run = [];
  };
  for (const child of Children.toArray(children)) {
    if (typeof child === "string" || typeof child === "number") run.push(child);
    else {
      close();
      out.push(child);
    }
  }
  close();
  if (count !== undefined && count !== null && count !== false)
    out.push(
      <span
        key="count"
        data-scope="status-item"
        data-part="count"
        aria-hidden={!spoken || undefined}
      >
        {count}
      </span>,
    );
  // A space between two parts of words, which a flex box draws as nothing and a reader reads as
  // a word's end: "todo 3", not "todo3". The icon says nothing, so none follows it.
  const iconed = icon !== undefined && icon !== null;
  return out.flatMap((part, at) => (at === 0 || (at === 1 && iconed) ? [part] : [" ", part]));
}

/** The root's own attributes. */
function root({
  variant = "bar",
  tone,
  strong,
  shrinks,
}: Pick<Drawn, "variant" | "tone" | "strong" | "shrinks">) {
  return {
    "data-scope": "status-item",
    "data-part": "root",
    "data-variant": variant,
    "data-tone": tone,
    "data-strong": strong ? "" : undefined,
    "data-shrinks": shrinks ? "" : undefined,
  } as const;
}

/**
 * **A status item that runs a command when pressed**: its icon, its words and its count, in a
 * variant. A trigger's `asChild` (a dialog's, a menu's) hands it its press too.
 */
export function StatusItem(props: StatusItemProps) {
  const {
    variant,
    tone,
    strong,
    shrinks,
    icon,
    count,
    children,
    className,
    ref,
    tooltip,
    ...rest
  } = props;
  const tip = usePressTip(props["aria-label"], tooltip, false, props.isDisabled === true);
  // The press the button family has too (`ui/pressable.tsx`): its native attributes, the keys a
  // handler does not stop going on (Shift F10 reaching the bar's menu), a disabled item with a
  // hint that keeps the focus and does nothing, and the element.
  const { native, aria, own } = pressable(rest, tip);
  return (
    <Tipped surface={tip.surface}>
      <AriaButton
        {...(aria as AriaButtonProps)}
        ref={ref}
        className={className}
        {...root({ variant, tone, strong, shrinks })}
        render={renderPressed(native, tip, own, discloses(rest))}
      >
        {parts({ icon, count, children }, false)}
      </AriaButton>
      {tip.reason}
    </Tipped>
  );
}

/**
 * **A status item that says something and runs nothing**: its icon, its words and its count,
 * as a cell of the bar. Its `tooltip` makes it a Tab stop, so the hint is reached by the keyboard
 * too (`ui/tooltip`).
 */
export function StatusCell({
  variant,
  tone,
  strong,
  shrinks,
  icon,
  count,
  children,
  tooltip,
  ...rest
}: Drawn & Omit<ComponentPropsWithRef<"span">, "style" | "children" | "title">) {
  return (
    <Tooltip tip={tooltip}>
      <span {...rest} {...root({ variant, tone, strong, shrinks })} data-static="">
        {parts({ icon, count, children }, true)}
      </span>
    </Tooltip>
  );
}

/**
 * **A verdict among an item's words**: words in a severity's colour beside the item's own (the
 * doctor's "2 warnings"), so the colour is never the only thing that says it. A `label` part.
 */
export function StatusVerdict({
  tone,
  children,
}: {
  tone: Extract<StatusTone, "waiting" | "failed">;
  children: ReactNode;
}) {
  return (
    <span data-scope="status-item" data-part="label" data-verdict={tone}>
      {children}
    </span>
  );
}
