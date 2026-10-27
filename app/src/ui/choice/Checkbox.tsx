// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The checkbox family** (#2907, spec #2897, ADR 0104): every box a person ticks, on React Aria's
 * `Checkbox` and `CheckboxGroup`, wrapped here once. A screen imports these and never
 * `react-aria-components` (`eslint.config.js`), and draws no `<input type="checkbox">` of its own
 * (`purlis/choice-family`, an error).
 *
 * - **A checkbox is a native box in its own label**: a click on its words ticks it as a click on
 *   its box does, Space ticks it from the keyboard, and a screen reader hears a checkbox named by
 *   its words, or by `aria-label` / `aria-labelledby` where a row draws its name. The id goes on
 *   the box, so a row's `<label for>` names it and a row's Reset can give it the focus back.
 * - **Drawn by its parts.** The root (the label) carries `data-scope="checkbox"`,
 *   `data-part="root"` and its `data-variant`; inside it, the `box` part (the mark ticked) and
 *   the `label` part (its words). `choice.css` draws the family default from them and React
 *   Aria's state attributes, and a theme's parts reach the same attributes (ADR 0103).
 * - **Variants**: `box` (the default, a square ticked) and `chip`: one of a few drawn small and
 *   side by side over the list they narrow (the Chats list's state chips), its tick in a slot
 *   kept for it, so a chip is told on by its tick and its ground, never by colour alone.
 * - **A tick never moves anything** (#2800, R153): the box, its tick and a chip's slot keep their
 *   size in every state.
 * - **Its hint is the shared tooltip** (`tooltip`, #2908): why an option is out of reach, or what
 *   it does that its words do not say.
 */
import {
  Children,
  createContext,
  isValidElement,
  useContext,
  useId,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type ReactNode,
  type Ref,
} from "react";
import {
  Checkbox as AriaCheckbox,
  type CheckboxProps as AriaCheckboxProps,
} from "react-aria-components";
import { Check, Minus } from "lucide-react";
import { Tooltip, type Tip } from "../tooltip/Tooltip";

/** The family's variants, in the order the manifest names them. */
export const CHECKBOX_VARIANTS = ["box", "chip"] as const;
export type CheckboxVariant = (typeof CHECKBOX_VARIANTS)[number];

export interface CheckboxProps extends Omit<
  AriaCheckboxProps,
  "className" | "style" | "children" | "render"
> {
  /** Its words, the `label` part. Left out, `aria-label` or `aria-labelledby` names the box. */
  children?: ReactNode;
  variant?: CheckboxVariant;
  /** A place for it in its view: margin, width, order. Never a look. */
  className?: string;
  style?: CSSProperties;
  /** A line under its words on what it does, which describes it: the `description` part. */
  description?: ReactNode;
  /** How many things it stands for, after its words: the `count` part (a chip's, #3484). It
   *  describes the box and is never part of its name, so the name stays its words. */
  count?: number;
  /** Its hint, the shared tooltip (#2908). */
  tooltip?: Tip;
  ref?: Ref<HTMLLabelElement>;
}

/** The mark inside the box: a tick, or a bar when only some of what it stands for are ticked. */
function Mark({ indeterminate }: { indeterminate: boolean }) {
  return (
    <span data-scope="checkbox" data-part="box" aria-hidden="true">
      {indeterminate ? (
        <Minus focusable="false" data-scope="checkbox" data-part="tick" />
      ) : (
        <Check focusable="false" data-scope="checkbox" data-part="tick" />
      )}
    </span>
  );
}

/** **A checkbox**: its box and its words, ticked by a press of either, or by Space. */
export function Checkbox({
  children,
  variant = "box",
  className,
  style,
  description,
  count,
  tooltip,
  ref,
  ...rest
}: CheckboxProps) {
  const id = useId();
  const described = description === undefined || description === null ? undefined : `${id}-says`;
  const counted = count === undefined ? undefined : `${id}-count`;
  const words = children !== undefined && children !== null && children !== false;
  // Named by its words alone: the line under them describes it, and is not its name.
  const named =
    words && rest["aria-label"] === undefined && rest["aria-labelledby"] === undefined
      ? `${id}-label`
      : rest["aria-labelledby"];
  const drawn = (
    <AriaCheckbox
      {...rest}
      aria-labelledby={named}
      aria-describedby={
        [rest["aria-describedby"], counted, described].filter(Boolean).join(" ") || undefined
      }
      ref={ref}
      className={className}
      style={style}
      data-scope="checkbox"
      data-part="root"
      data-variant={variant}
    >
      {({ isIndeterminate }) => (
        <>
          <Mark indeterminate={isIndeterminate} />
          {words ? (
            <span data-scope="checkbox" data-part="label" id={`${id}-label`}>
              {children}
            </span>
          ) : null}
          {counted !== undefined && (
            <span data-scope="checkbox" data-part="count" id={counted}>
              {count}
            </span>
          )}
          {described !== undefined && (
            <span data-scope="checkbox" data-part="description" id={described}>
              {description}
            </span>
          )}
        </>
      )}
    </AriaCheckbox>
  );
  return tooltip === undefined || tooltip === null ? (
    drawn
  ) : (
    <Tooltip tip={tooltip}>{drawn}</Tooltip>
  );
}

export interface CheckboxGroupProps {
  children?: ReactNode;
  id?: string;
  "aria-label"?: string;
  "aria-labelledby"?: string;
  "aria-describedby"?: string;
  /** A place for it in its view. Never a look. */
  className?: string;
  style?: CSSProperties;
  /**
   * **One Tab stop, the arrows between its boxes** (the chips over a list, #1687; the APG
   * toolbar's keys): Tab reaches the box the keyboard was last on, or the first one that can be
   * used when that one cannot or is gone; the arrow keys move along the row, Home and End go to
   * its ends; and Tab leaves it in one press, so a list's filter costs the keyboard one stop
   * however many chips it has. Its boxes are {@link RovingCheckbox}es.
   */
  roving?: boolean;
  ref?: Ref<HTMLDivElement>;
}

/** The keys that move along a roving row of boxes, by a step. */
const ALONG: Record<string, number> = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 };

/** The keys that go to a roving row's ends. */
const ENDS: Record<string, "first" | "last"> = { Home: "first", End: "last" };

/** The places of a roving group's boxes that can be used, in the order they are drawn: its
 *  direct children's, which is how a roving group is written (each box a child of it). */
function usable(children: ReactNode): number[] {
  return Children.toArray(children).flatMap((child) => {
    if (!isValidElement(child)) return [];
    const { at, isDisabled } = child.props as { at?: unknown; isDisabled?: unknown };
    return typeof at === "number" && isDisabled !== true ? [at] : [];
  });
}

/**
 * **A group of checkboxes**: any of a few, each ticked on its own, named as one (`aria-label`,
 * `aria-labelledby`). Each box holds its own value and is ticked by its own `isSelected`, so two
 * options may stand for one value (two owners' `api`, D-DS3e-10).
 */
export function CheckboxGroup({ children, roving = false, ref, ...rest }: CheckboxGroupProps) {
  const own = useRef<HTMLDivElement | null>(null);
  // The box the keyboard is on, by place: Tab comes back to it.
  const [at, setAt] = useState(0);
  const boxes = () => [
    ...(own.current?.querySelectorAll<HTMLInputElement>('input[type="checkbox"]') ?? []),
  ];
  // The box that holds the Tab stop: the one the keyboard was last on while it can be used,
  // else the first that can.
  // Read from the boxes the group holds directly; a row whose boxes are wrapped (a tooltip
  // around each) keeps the stop where the keyboard was, the first box until then.
  const open = usable(children);
  const stop = open.length === 0 ? at : open.includes(at) ? at : open[0];
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!roving) return;
    const step = ALONG[event.key];
    const end = ENDS[event.key];
    if (step === undefined && end === undefined) return;
    const all = boxes().filter((one) => !one.disabled);
    const from = all.indexOf(document.activeElement as HTMLInputElement);
    if (from < 0 || all.length === 0) return;
    event.preventDefault();
    const to =
      end === "first"
        ? 0
        : end === "last"
          ? all.length - 1
          : (from + (step ?? 0) + all.length) % all.length;
    all[to].focus();
  };
  const onFocus = () => {
    if (!roving) return;
    const now = boxes().indexOf(document.activeElement as HTMLInputElement);
    if (now >= 0) setAt(now);
  };
  return (
    <div
      {...rest}
      ref={(element) => {
        own.current = element;
        if (typeof ref === "function") ref(element);
        else if (ref) ref.current = element;
      }}
      role="group"
      data-scope="checkbox-group"
      data-roving={roving ? "" : undefined}
      onKeyDown={onKeyDown}
      onFocus={onFocus}
    >
      <RovingStop.Provider value={roving ? (stop ?? -1) : undefined}>
        {children}
      </RovingStop.Provider>
    </div>
  );
}

/** The place of the box that holds a roving group's one Tab stop; unset outside one. */
const RovingStop = createContext<number | undefined>(undefined);

/**
 * **A box of a roving group** (`CheckboxGroup roving`): a {@link Checkbox} that takes the
 * group's Tab stop when it is the one the keyboard was last on. `at` is its place in the row.
 */
export function RovingCheckbox({ at, ...props }: CheckboxProps & { at: number }) {
  const stop = useContext(RovingStop);
  return <Checkbox {...props} excludeFromTabOrder={stop !== undefined && stop !== at} />;
}
