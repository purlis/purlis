// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The card family** (#2912, spec #2897, ADR 0104): a box one step off the surface it stands
 * on, holding an ask, a question or a group of rows. A screen imports this and never draws its
 * own box (`purlis/card-family`).
 *
 * - **Drawn by its parts.** The root carries `data-scope="card"`, `data-part="root"` and its
 *   `data-variant`; its first line, where it has one, is the `title` part. `card.css` draws the
 *   family default and a theme's parts reach the same attributes (ADR 0103).
 * - **Variants**: `raised` (the default) and `ask`, a card that asks the person something, edged
 *   in the waiting colour in every theme: no theme can take that edge away (ADR 0097 §6), as it
 *   cannot take Allow's.
 * - **Purlis draws it the Figma way**: the window's fill on its panel, corners 10, no hairline
 *   (#2804, R25); Purlis Classic, on its raised ground with the subtle hairline.
 * - **A place, never a look**: `className` gives it a margin, a width or an order.
 */
import { createElement, type ComponentPropsWithRef, type ReactNode, type Ref } from "react";

/** The family's variants, in the order the manifest names them. */
export const CARD_VARIANTS = ["raised", "ask"] as const;
export type CardVariant = (typeof CARD_VARIANTS)[number];

/** The elements a card is drawn as: a box, an item of a list, a line, a section of a page. */
type CardElement = "div" | "li" | "p" | "section" | "article";

export function Card({
  as = "div",
  variant = "raised",
  children,
  ...rest
}: Omit<ComponentPropsWithRef<"div">, "style" | "ref"> & {
  ref?: Ref<HTMLElement>;
  as?: CardElement;
  variant?: CardVariant;
  children?: ReactNode;
}) {
  return createElement(
    as,
    {
      ...rest,
      "data-scope": "card",
      "data-part": "root",
      "data-variant": variant,
    },
    children,
  );
}

/** **A card's first line**: whose it is, what it asks. */
export function CardTitle({
  as = "p",
  children,
  ...rest
}: Omit<ComponentPropsWithRef<"p">, "style"> & {
  as?: "p" | "h3" | "h4" | "strong";
  children: ReactNode;
}) {
  return createElement(as, { ...rest, "data-scope": "card", "data-part": "title" }, children);
}
