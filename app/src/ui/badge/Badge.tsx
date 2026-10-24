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
 *   R101) and `hot` (what needs the person now: the needs-you colours, R120).
 * - **Words, not controls.** Neither presses anything. A badge with a hint is the tooltip's target
 *   (`<Tooltip tip>` around it), which makes it a Tab stop so the hint is read without a pointer;
 *   its ref and props reach the element for that.
 * - **A count says what it counts.** Its number is drawn and hidden from a screen reader where
 *   `said` gives the sentence it is heard as instead ("3 chats need you"); a count of nothing is
 *   drawn by no one: a zero is not a badge.
 * - **A place, never a look**: `className` gives it a margin, a position or an order.
 */
import type { ComponentPropsWithRef, ReactNode } from "react";
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
  said,
  children,
  ...rest
}: Span & {
  variant?: CountVariant;
  /** What a screen reader hears in place of the number, where the number alone says too
   *  little: drawn after it, visually hidden. */
  said?: string;
  /** The number, or what stands for it ("2 ↑1"). */
  children: ReactNode;
}) {
  return (
    <span {...rest} data-scope="count" data-part="root" data-variant={variant}>
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
