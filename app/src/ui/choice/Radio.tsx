// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The one-of-a-few families** (#2907, spec #2897, ADR 0104): the radio group, the segmented
 * control and the option cards, each on React Aria's `RadioGroup` and `Radio`, wrapped here once.
 * A screen imports these and never `react-aria-components` (`eslint.config.js`), and draws no
 * `<input type="radio">`, `role="radio"` or `role="radiogroup"` of its own
 * (`purlis/choice-family`, an error).
 *
 * All three are one radio group to the keyboard and to a screen reader: **one Tab stop**, the
 * arrow keys move the pick along (the WAI-ARIA radio pattern, React Aria's own), and each option
 * is a native radio named by its words. What differs is how a pick is shown:
 *
 * - **A radio group** (`RadioGroup`, `Radio`): a column of options, each its `mark` (the circle
 *   picked), its `label` and, under them, its `description` (a line on what it does).
 * - **A segmented control** (`SegmentedControl`, `Segment`): a few views of one thing, side by
 *   side on one ground (a level of Settings, a side view's scope). The chosen segment stands on
 *   the window's ground with a 1px edge under it; its weight never changes (#2800, #2868·18).
 * - **Option cards** (`OptionCards`, `OptionCard`): options a person compares before picking
 *   (Start a chat's harnesses, #2918; R24), each a card of the same height as the tallest, its
 *   `icon` and `title` on the first line and its `detail` under them, and **no radio circle**.
 *   The chosen card is told by its wash, its accent edge and its `check`, never by colour alone
 *   (WCAG 1.4.1). A `tile` is a card that is only its icon (a persona's mark), named by its
 *   `aria-label`.
 *
 * **A pick never moves anything** (#2800, R153): every option keeps its size in every state, and
 * a card's check has its slot whether it is shown or not.
 */
import { useId, type CSSProperties, type ReactElement, type ReactNode, type Ref } from "react";
import {
  Radio as AriaRadio,
  RadioGroup as AriaRadioGroup,
  type RadioGroupProps as AriaRadioGroupProps,
  type RadioProps as AriaRadioProps,
} from "react-aria-components";
import { Check } from "lucide-react";
import { Tooltip, type Tip } from "../tooltip/Tooltip";

/** What every group of the three takes beside React Aria's own props. */
interface Group extends Omit<AriaRadioGroupProps, "className" | "style" | "children" | "render"> {
  children?: ReactNode;
  /** A place for it in its view: margin, width, order. Never a look. */
  className?: string;
  style?: CSSProperties;
  ref?: Ref<HTMLDivElement>;
}

/** What every option of the three takes beside React Aria's own props. */
interface Option extends Omit<AriaRadioProps, "className" | "style" | "children" | "render"> {
  /** A place for it in its view. Never a look. */
  className?: string;
  style?: CSSProperties;
  /** Its hint, the shared tooltip (#2908): why it is out of reach, or what its words leave out. */
  tooltip?: Tip;
  ref?: Ref<HTMLLabelElement>;
}

/** An option drawn with its hint, when it has one. */
function tipped(tooltip: Tip | undefined, drawn: ReactElement) {
  return tooltip === undefined || tooltip === null ? (
    drawn
  ) : (
    <Tooltip tip={tooltip}>{drawn}</Tooltip>
  );
}

// --- The radio group --------------------------------------------------------------------------

export type RadioGroupProps = Group;

/** **A radio group**: one of a few, a column of options with a line on what each does. */
export function RadioGroup({ children, ...rest }: RadioGroupProps) {
  return (
    <AriaRadioGroup {...rest} data-scope="radio-group">
      {children}
    </AriaRadioGroup>
  );
}

export interface RadioProps extends Option {
  /** Its words, the `label` part. */
  children?: ReactNode;
  /** What is drawn before its words and adds nothing to its name: a persona's mark. */
  lead?: ReactNode;
  /** A line under it on what it does, which describes it: the `description` part. */
  description?: ReactNode;
}

/** **One option of a radio group**: its mark, its words and the line under them. */
export function Radio({ children, lead, description, tooltip, ...rest }: RadioProps) {
  const id = useId();
  const described = description === undefined || description === null ? undefined : `${id}-says`;
  return tipped(
    tooltip,
    <AriaRadio
      {...rest}
      // Named by its words alone: the line under them describes it, and is not its name.
      aria-labelledby={
        rest["aria-label"] === undefined ? (rest["aria-labelledby"] ?? `${id}-label`) : undefined
      }
      aria-describedby={
        [rest["aria-describedby"], described].filter(Boolean).join(" ") || undefined
      }
      data-scope="radio"
      data-part="root"
    >
      <span data-scope="radio" data-part="mark" aria-hidden="true" />
      <span data-scope="radio" data-part="label" id={`${id}-label`}>
        {lead}
        {children}
      </span>
      {described !== undefined && (
        <span data-scope="radio" data-part="description" id={described}>
          {description}
        </span>
      )}
    </AriaRadio>,
  );
}

// --- The segmented control --------------------------------------------------------------------

/** The segmented control's sizes, as the button family's: `sm` the chrome's height, never under 24 px, and `md` the control's. */
export const SEGMENTED_SIZES = ["sm", "md"] as const;
export type SegmentedSize = (typeof SEGMENTED_SIZES)[number];

export interface SegmentedControlProps extends Group {
  size?: SegmentedSize;
}

/** **A segmented control**: a few views of one thing, side by side, one chosen. */
export function SegmentedControl({
  children,
  size = "md",
  orientation = "horizontal",
  ...rest
}: SegmentedControlProps) {
  return (
    <AriaRadioGroup
      {...rest}
      orientation={orientation}
      data-scope="segmented-control"
      data-part="root"
      data-size={size}
    >
      {children}
    </AriaRadioGroup>
  );
}

export interface SegmentProps extends Option {
  /** Its words, the `label` part. */
  children?: ReactNode;
  /** How many things it stands for, after its words: the `count` part. Never the only thing that
   *  tells two segments apart, and said in its name too. */
  count?: number;
}

/** **One segment** of a segmented control. */
export function Segment({ children, count, tooltip, ...rest }: SegmentProps) {
  const id = useId();
  return tipped(
    tooltip,
    <AriaRadio
      {...rest}
      // Named by its words and its count, said apart.
      aria-labelledby={
        rest["aria-label"] === undefined
          ? (rest["aria-labelledby"] ??
            (count === undefined ? `${id}-label` : `${id}-label ${id}-count`))
          : undefined
      }
      data-scope="segment"
      data-part="root"
    >
      <span data-scope="segment" data-part="label" id={`${id}-label`}>
        {children}
      </span>
      {count !== undefined && (
        <span data-scope="segment" data-part="count" id={`${id}-count`}>
          {count}
        </span>
      )}
    </AriaRadio>,
  );
}

// --- The option cards -------------------------------------------------------------------------

/** The option card's variants, in the order the manifest names them. */
export const OPTION_CARD_VARIANTS = ["card", "tile"] as const;
export type OptionCardVariant = (typeof OPTION_CARD_VARIANTS)[number];

export interface OptionCardsProps extends Group {
  /** `tile`: a row of square tiles that wraps, each only its icon. */
  variant?: OptionCardVariant;
}

/** **Option cards**: one of a few, each a card of the same height, compared before picking. */
export function OptionCards({ children, variant = "card", ...rest }: OptionCardsProps) {
  return (
    <AriaRadioGroup {...rest} data-scope="option-cards" data-variant={variant}>
      {children}
    </AriaRadioGroup>
  );
}

export interface OptionCardProps extends Option {
  /** Its name, the `title` part. Left out for a tile, which `aria-label` names. */
  title?: ReactNode;
  /** Its mark, before its title: the `icon` part. A tile is only this. */
  icon?: ReactNode;
  /** What it means, under its title: the `detail` part, which describes it. */
  detail?: ReactNode;
  variant?: OptionCardVariant;
}

/** **One option card**: its icon and title, its detail under them, and its check when chosen. */
export function OptionCard({
  title,
  icon,
  detail,
  variant = "card",
  tooltip,
  ...rest
}: OptionCardProps) {
  const id = useId();
  const described = detail === undefined || detail === null ? undefined : `${id}-says`;
  const titled = title !== undefined && title !== null;
  return tipped(
    tooltip,
    <AriaRadio
      {...rest}
      // Named by its title alone: its detail describes it, and is not its name.
      aria-labelledby={
        titled && rest["aria-label"] === undefined
          ? (rest["aria-labelledby"] ?? `${id}-title`)
          : rest["aria-labelledby"]
      }
      aria-describedby={
        [rest["aria-describedby"], described].filter(Boolean).join(" ") || undefined
      }
      data-scope="option-card"
      data-part="root"
      data-variant={variant}
    >
      {icon !== undefined && icon !== null && (
        <span data-scope="option-card" data-part="icon" aria-hidden="true">
          {icon}
        </span>
      )}
      {title !== undefined && title !== null && (
        <span data-scope="option-card" data-part="title" id={`${id}-title`}>
          {title}
        </span>
      )}
      {variant === "card" && (
        <Check data-scope="option-card" data-part="check" aria-hidden="true" focusable="false" />
      )}
      {described !== undefined && (
        <span data-scope="option-card" data-part="detail" id={described}>
          {detail}
        </span>
      )}
    </AriaRadio>,
  );
}
