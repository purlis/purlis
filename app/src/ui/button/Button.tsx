// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The button family** (#2904, spec #2897, ADR 0104): every button, icon button and button
 * drawn as a link in the window, on React Aria's `Button` and `ToggleButton`, wrapped here once.
 * A screen imports these and never `react-aria-components` (`eslint.config.js`).
 *
 * - **Drawn by its parts.** The root carries `data-scope="button"`, `data-part="root"`, its
 *   `data-variant` and its `data-size`; its words are the `label` part and its icon the `icon`
 *   part. `button.css` draws the family default from them and a theme's parts reach the same
 *   attributes (ADR 0103). A screen gives a button a place (`className` for margin, width,
 *   order), never a look.
 * - **Variants**: `secondary` (the plain button, the default), `primary` (the answer purlis
 *   suggests), `ghost`, `danger` (what cannot be undone from purlis, D-1719-3), `soft-accent`,
 *   `warning-soft`, `link`, `quiet` (the ghost with quiet words: a strip's end mark, a crumb, a
 *   pane's corner, #3214), and `grants`: an answer that grants, an Allow or an Accept
 *   (ADR 0097 §6), which leads with the open lock the family draws itself and never takes the
 *   primary look.
 * - **Sizes**: `sm`, `md` (the default) and `lg`, which is only for the control beside a list's
 *   main filter field. A row of buttons takes one size (`ButtonRow`), so a mixed row cannot be
 *   built. **`inline`** draws a button as words in a line of words, a breadcrumb's crumb (#3264):
 *   as tall as the line, with no box padding, its words cut to an ellipsis where the line
 *   squeezes it and underlined under the pointer and the keyboard, as a link's are. Words only:
 *   an inline button is a box of words, and draws no icon beside them.
 * - **Behaviour is React Aria's**: press by pointer, Enter and Space, focus on press in WebKit,
 *   a visible focus ring only from the keyboard. Every button is in the tab sequence, which
 *   WebKit leaves a button out of without `tabIndex` (#190).
 * - **Its hint is the shared tooltip** (#2908, `ui/tooltip`), never a native `title`: `tooltip`
 *   on a `Button` or a `ToggleButton`. **An icon button's tooltip is its name** (its
 *   `aria-label`), with its `tooltip` as a second line under it, so what the tooltip shows and
 *   what a screen reader hears always agree; a hint that only repeats a button's name is shown
 *   and not heard twice. **A disabled button with a hint keeps the focus** and says it is
 *   unavailable (`aria-disabled`), so the hint, which says why, is reached by the keyboard and
 *   the pointer alike, and nothing presses it: nor opens the menu or popover it is the trigger
 *   of (#3116).
 * - **The key that presses it** (R33, #3377): `shortcut` draws the key after its words, in the
 *   `shortcut` part, a `kbd` small and mono. It is drawn by the stylesheet from its `data-key`
 *   and hidden from a screen reader, so the button's name and words stay its words alone; the
 *   key is said by `aria-keyshortcuts`, which the caller gives. `shortcutOff` keeps its room,
 *   undrawn, while the key does not press it, so a choice widens nothing (R163).
 */
import {
  Children,
  createContext,
  useContext,
  useState,
  type ComponentPropsWithRef,
  type CSSProperties,
  type ReactNode,
  type Ref,
} from "react";
import {
  Button as AriaButton,
  ToggleButton as AriaToggleButton,
  type ButtonProps as AriaButtonProps,
  type ToggleButtonProps as AriaToggleButtonProps,
} from "react-aria-components";
import { LockOpen } from "lucide-react";
import { pressable, renderPressed, usePressTip, type Native } from "../pressable";
import { Tipped, type Tip } from "../tooltip/Tooltip";

/** The family's variants, in the order the manifest names them. */
export const BUTTON_VARIANTS = [
  "secondary",
  "primary",
  "ghost",
  "danger",
  "grants",
  "soft-accent",
  "warning-soft",
  "link",
  "quiet",
] as const;
export type ButtonVariant = (typeof BUTTON_VARIANTS)[number];

/** The family's sizes: 24, 28 and 32 px at the default text size and density (`button.css`). */
export const BUTTON_SIZES = ["sm", "md", "lg"] as const;
export type ButtonSize = (typeof BUTTON_SIZES)[number];

/** What every member of the family takes beside React Aria's own props. */
interface Drawn extends Native {
  variant?: ButtonVariant;
  /** Ignored inside a `ButtonRow`, whose size every button in it takes. */
  size?: ButtonSize;
  /**
   * **Words in a line of words** (#3264): as tall as the line, no box padding, cut to an
   * ellipsis where the line squeezes it (a breadcrumb's crumb). A target inline in text is
   * exempt from the target size (WCAG 2.5.8).
   */
  inline?: boolean;
  /** A place for the button in its view: margin, width, flex, order. Never a look. */
  className?: string;
  style?: CSSProperties;
  /** An icon before the words: the family's `icon` part. */
  icon?: ReactNode;
  /**
   * **Its hint**, the shared tooltip (#2908): what the button does that its words do not say, or
   * why it cannot be pressed now. On an icon button, a second line under its name.
   */
  tooltip?: Tip;
  /** The key that presses it, as a hint after its words: the family's `shortcut` part (R33). */
  shortcut?: string;
  /** The hint's room is kept and nothing drawn in it: the key does not press it now. */
  shortcutOff?: boolean;
  ref?: Ref<HTMLButtonElement>;
  /** `data-*` attributes reach the element as they are. */
  [data: `data-${string}`]: string | number | boolean | undefined;
}

export interface ButtonProps
  extends Drawn, Omit<AriaButtonProps, "className" | "style" | "children" | "render"> {
  children?: ReactNode;
}

export interface ToggleButtonProps
  extends
    Omit<Drawn, "variant">,
    Omit<AriaToggleButtonProps, "className" | "style" | "children" | "render"> {
  /**
   * **A toggle is a plain or a ghost button** (ADR 0097 §6, #2958): an answer that grants is
   * pressed once, never held pressed, and the suggested answer, a danger answer and the soft,
   * warning and link looks are never pressed as one. The parts manifest draws only these two
   * pressed (`DRAWN_ONLY`, #3255).
   */
  variant?: ToggleVariant;
  children?: ReactNode;
}

/** The variants a toggle is drawn in. */
export type ToggleVariant = Extract<ButtonVariant, "secondary" | "ghost">;

export type IconButtonProps = Omit<ButtonProps, "children" | "icon" | "variant"> & {
  /** The icon, which is all the button shows. */
  icon: ReactNode;
  /** Not `grants`: an answer that grants says its words beside its lock (ADR 0097 §6). */
  variant?: Exclude<ButtonVariant, "grants">;
} & ({ "aria-label": string } | { "aria-labelledby": string });

/** The size a `ButtonRow` hands every button in it. */
const RowSize = createContext<ButtonSize | undefined>(undefined);

/**
 * **A row of buttons** (#2897: one size per row; #1855: a row that wraps). Every button in it
 * takes the row's size, whatever its own `size` says. `wraps` lets the buttons go onto as many
 * lines as they need, where the room may be narrower than they are (a Notice in a narrow pane),
 * and a button that alone is wider than the row breaks its words instead of being clipped.
 */
export function ButtonRow({
  size = "md",
  wraps = true,
  children,
  ...rest
}: Omit<ComponentPropsWithRef<"div">, "style"> & {
  size?: ButtonSize;
  wraps?: boolean;
}) {
  return (
    <RowSize.Provider value={size}>
      <div {...rest} data-scope="button-row" data-size={size} data-wraps={wraps ? "" : undefined}>
        {children}
      </div>
    </RowSize.Provider>
  );
}

/**
 * The words of a button as its `label` part: each run of text, as a browser makes each run an
 * anonymous item of a flex box, so a label is laid out exactly as the bare text was. Elements
 * (an icon, a count, a key) stay where they are.
 */
function labelled(children: ReactNode): ReactNode {
  const out: ReactNode[] = [];
  let run: (string | number)[] = [];
  const close = () => {
    if (run.length === 0) return;
    out.push(
      <span key={`label-${out.length}`} data-scope="button" data-part="label">
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
  return out;
}

function Icon({ children }: { children: ReactNode }) {
  return (
    <span data-scope="button" data-part="icon" aria-hidden="true">
      {children}
    </span>
  );
}

/**
 * The key that presses a button, after its words (R33). Drawn from `data-key` by `button.css`,
 * so it is never part of the button's words or name.
 */
function Shortcut({ keys, off }: { keys: string; off: boolean }) {
  return (
    <kbd
      data-scope="button"
      data-part="shortcut"
      data-key={keys}
      data-off={off ? "" : undefined}
      aria-hidden="true"
    />
  );
}

/** The lock an answer that grants leads with (ADR 0097 §6): the cue that is not colour. */
function GrantsLock() {
  return (
    <Icon>
      <LockOpen aria-hidden="true" focusable="false" />
    </Icon>
  );
}

function useDrawn(
  props: Drawn & { children?: ReactNode; isDisabled?: boolean; "aria-label"?: string },
  iconOnly = false,
) {
  const rowSize = useContext(RowSize);
  const {
    variant = "secondary",
    size: ownSize = "md",
    inline = false,
    icon,
    children,
    className,
    style,
    ref,
    tooltip,
    shortcut,
    shortcutOff = false,
    ...rest
  } = props;
  const tip = usePressTip(props["aria-label"], tooltip, iconOnly, props.isDisabled === true);
  const size = rowSize ?? ownSize;
  // The press every pressable family shares (`ui/pressable.tsx`): the native attributes, the
  // keys a handler does not stop going on, a disabled button with a hint that keeps the focus
  // and does nothing, and the element.
  const { native, aria, own } = pressable(rest, tip);
  const content = (
    <>
      {variant === "grants" ? <GrantsLock /> : null}
      {icon !== undefined && icon !== null ? <Icon>{icon}</Icon> : null}
      {iconOnly ? null : labelled(children)}
      {iconOnly || shortcut === undefined || shortcut === "" ? null : (
        <Shortcut keys={shortcut} off={shortcutOff} />
      )}
    </>
  );
  const drawn = {
    ...aria,
    ref,
    className,
    style,
    "data-scope": "button",
    "data-part": "root",
    "data-variant": variant,
    "data-size": size,
    "data-icon-only": iconOnly ? "" : undefined,
    "data-inline": inline ? "" : undefined,
    // #190, #1700, #2908: `ui/pressable.tsx`.
    render: renderPressed(native, tip, own),
    children: content,
  };
  return { drawn, surface: tip.surface, reason: tip.reason };
}

/** **A button**: words, an icon before them if it has one, in a variant and a size. */
export function Button(props: ButtonProps) {
  const { drawn, surface, reason } = useDrawn(props);
  return (
    <Tipped surface={surface}>
      <AriaButton {...(drawn as AriaButtonProps)} />
      {reason}
    </Tipped>
  );
}

/**
 * **An icon button**: square at its size, named by its `aria-label`, which its tooltip shows;
 * its `tooltip` is a second line under the name.
 */
export function IconButton(props: IconButtonProps) {
  const { drawn, surface, reason } = useDrawn(props as Drawn, true);
  return (
    <Tipped surface={surface}>
      <AriaButton {...(drawn as AriaButtonProps)} />
      {reason}
    </Tipped>
  );
}

/** **A toggle button**: pressed or not (`aria-pressed`), told by its edge and its ground. */
export function ToggleButton(given: ToggleButtonProps) {
  // Held here when the call does not hold it, so an unavailable toggle, whose `onChange` is
  // dropped with its other handlers, never flips: React Aria's own state would.
  const [held, hold] = useState(given.defaultSelected ?? false);
  const props: ToggleButtonProps =
    given.isSelected !== undefined
      ? given
      : {
          ...given,
          defaultSelected: undefined,
          isSelected: held,
          onChange: (selected: boolean) => {
            hold(selected);
            given.onChange?.(selected);
          },
        };
  const { drawn, surface, reason } = useDrawn(props);
  return (
    <Tipped surface={surface}>
      <AriaToggleButton {...(drawn as AriaToggleButtonProps)} />
      {reason}
    </Tipped>
  );
}
