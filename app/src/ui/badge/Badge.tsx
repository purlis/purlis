// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The badge and count family** (#2912, spec #2897, ADR 0104): a word or two beside what it is
 * about (a badge), and a number on a pill beside what it counts (a count). A screen imports these
 * and never draws its own pill (`purlis/badge-family`).
 *
 * - **Drawn by its parts.** A badge is `data-scope="badge" data-part="root"`, a count
 *   `data-scope="count" data-part="root"`, each with its `data-variant`; counts drawn as one pill
 *   stand in a `CountGroup` (`data-scope="count" data-part="group"`). `badge.css` draws the family
 *   default and a theme's parts reach the same attributes (ADR 0103).
 * - **A badge's variants**: `neutral`, `accent` (where a value comes from), `waiting` ("Waiting
 *   for you", "deprecated": the amber wash, R66, R62) and `ok` ("Confirmed": the ok words, R66).
 *   **A count's**: `neutral`, `accent`, `waiting` (what waits: the amber wash, R46,
 *   R101) and `hot` (what needs the person now: the needs-you colours, R120). A count is drawn
 *   at one size, or `sm` on a mark's corner (#5189).
 * - **Words, not controls.** Neither presses anything. A badge with a hint is the tooltip's target
 *   (`<Tooltip tip>` around it), which makes it a Tab stop so the hint is read without a pointer;
 *   its ref and props reach the element for that.
 * - **A count says what it counts.** Its number is drawn and hidden from a screen reader where
 *   `said` gives the sentence it is heard as instead ("3 chats need you"); a count of nothing is
 *   drawn by no one: a zero is not a badge.
 * - **A place, never a look**: `className` gives it a margin, a position or an order.
 */
import { useState, type ComponentPropsWithRef, type ReactNode } from "react";
import { drawsWords } from "../tooltip/Tooltip";

/** The badge's variants, in the order the manifest names them. */
export const BADGE_VARIANTS = ["neutral", "accent", "waiting", "ok"] as const;
export type BadgeVariant = (typeof BADGE_VARIANTS)[number];

/** The count's variants, in the order the manifest names them. */
export const COUNT_VARIANTS = ["neutral", "accent", "waiting", "hot"] as const;
export type CountVariant = (typeof COUNT_VARIANTS)[number];

type Span = Omit<ComponentPropsWithRef<"span">, "children" | "style">;

/** **A badge**: a word or two beside what it is about. */
export function Badge({
  variant = "neutral",
  children,
  ...rest
}: Span & { variant?: BadgeVariant; children: ReactNode }) {
  return (
    <span {...rest} data-scope="badge" data-part="root" data-variant={variant}>
      {children}
    </span>
  );
}

drawsWords(Badge);

/** **The status of a host waiting for Confirm, or confirmed** (R66): the two words the table of
 *  your hosts says, each in its badge. */
export function HostStatus({ waiting }: { waiting: boolean }) {
  return waiting ? (
    <Badge variant="waiting">Waiting for you</Badge>
  ) : (
    <Badge variant="ok">Confirmed</Badge>
  );
}

/** **A count**: a number on a pill. */
export function Count({
  variant = "neutral",
  size,
  said,
  children,
  ...rest
}: Span & {
  variant?: CountVariant;
  /** `sm`: the small pill that stands on a mark's corner (a pane's bell, #5189), a size of its
   *  own that a theme's room for a count never reaches. */
  size?: "sm";
  /** What a screen reader hears in place of the number, where the number alone says too
   *  little: drawn after it, visually hidden. */
  said?: string;
  /** The number, or what stands for it ("2 ↑1"). */
  children: ReactNode;
}) {
  return (
    <span {...rest} data-scope="count" data-part="root" data-variant={variant} data-size={size}>
      {said === undefined ? (
        children
      ) : (
        <>
          <span aria-hidden="true">{children}</span>
          <span className="sr-only">{said}</span>
        </>
      )}
    </span>
  );
}

/** The most a round count draws as a figure; above it, `9+`. */
const MOST_DRAWN = 9;

/** **What a round count draws for `count`** (ADR 0121, amended 2026-11-12, decision 7): one
 *  character, 1 to 9, then `9+`, so the circle never changes size. What is said stays the real
 *  number. */
export const capped = (count: number): string =>
  count > MOST_DRAWN ? `${MOST_DRAWN}+` : String(count);

/** Which way the drawn value last changed, and whether that was an even or odd change: the two
 *  keyframe names alternate, so each change starts the slide again without drawing the element
 *  again (a rail tab must not remount). Nothing until it first changes. */
type Slid = { drawn: string; count: number; slide?: "a" | "b"; dir?: "up" | "down" };

function useSlid(count: number): Slid {
  const drawn = capped(count);
  const [slid, setSlid] = useState<Slid>({ drawn, count });
  // Adjusted while rendering, as React has a state follow a prop: no effect, no second paint.
  if (slid.count !== count) {
    const moved = slid.drawn !== drawn;
    const next: Slid = moved
      ? {
          drawn,
          count,
          slide: slid.slide === "a" ? "b" : "a",
          dir: count > slid.count ? "up" : "down",
        }
      : { ...slid, count };
    setSlid(next);
    return next;
  }
  return slid;
}

/**
 * **A round count** (ADR 0121, amended 2026-11-12, decisions 6 and 7; #6342): every count of what
 * waits on the person (the ✋'s, a project tab's, an Asks chip's) as one small circle in the ✋'s
 * colour, holding one character ({@link capped}). Nothing is drawn for a zero.
 *
 * - `at="mark"`: 16px on a mark's corner (the ✋, a rail tab), ringed 2px in the ground under it
 *   (`--count-ground`, the window's frame unless its host says another). `at="inline"`: 14px,
 *   after words (a chip).
 * - **Its character is drawn by the stylesheet**, never a word of its own, so a host whose name
 *   is its own (a button) keeps it. **What is said is the real number** (120, never "9+"): `said`
 *   is read in its place where given; without it the count is hidden and its host says the
 *   number (the ✋'s description, a tab's).
 * - **A changed drawn value slides once** (`--motion-duration-leave`); the same drawn value (9+
 *   to 9+) does not, and under reduced motion it just changes.
 */
export function RoundCount({
  count,
  at,
  said,
  ...rest
}: Omit<Span, "aria-hidden"> & {
  count: number;
  at: "mark" | "inline";
  /** What a screen reader hears for it: the real number, or a sentence that holds it. */
  said?: string;
}) {
  const { drawn, slide, dir } = useSlid(count);
  if (count <= 0) return null;
  return (
    <span
      {...rest}
      data-scope="count"
      data-part="round"
      data-at={at}
      data-drawn={drawn}
      data-slide={slide}
      data-slide-dir={dir}
      aria-hidden={said === undefined ? "true" : undefined}
    >
      {said !== undefined && <span className="sr-only">{said}</span>}
    </span>
  );
}

/** **Counts drawn as one pill** (a tab's chats beside those that need the person). */
export function CountGroup({
  children,
  ...rest
}: Omit<ComponentPropsWithRef<"span">, "style"> & { children: ReactNode }) {
  return (
    <span {...rest} data-scope="count" data-part="group">
      {children}
    </span>
  );
}
