// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A card pressed whole** (#3655, the card family): a door on the welcome, a repo to get. One
 * button the size of the card, on React Aria's `Button` through the press every pressable family
 * shares (`ui/pressable.tsx`), so it is pressed by the pointer, Enter and Space, keeps WebKit's
 * tab sequence (#190) and draws its ring from the keyboard only.
 *
 * - **Named by its title and what follows it, described by its line**: a screen reader reads the
 *   act first and the explanation after it. Its name is only words it shows, so what a person
 *   reads on it and what they say to a voice control agree (WCAG 2.5.3, Label in Name).
 * - **Drawn by its parts.** Its root is `data-scope="card"`, `data-part="pressable"` and its
 *   `data-variant`; its `mark`, `title` and `line` are parts of their own. `card.css` draws them.
 *   It is a part of its own, not the card's `root`, so a theme that sets its cards flat on their
 *   panel (Purlis) never takes a pressed card's edge away; it is drawn in the `control.*` shades
 *   every theme sets, as a thing that is pressed.
 * - **Variants**: `door` (the default), a mark, a title and a line in a column on the control
 *   ground; `pick`, one line on the surface under it, a pick from a list.
 * - **A place, never a look**: `className` gives it a margin, a width or an order.
 */
import { Children, useId, type ReactNode, type Ref } from "react";
import { Button as AriaButton, type ButtonProps as AriaButtonProps } from "react-aria-components";
import { pressable, renderPressed, type Native } from "../pressable";

/** The pressable card's variants. */
export const CARD_BUTTON_VARIANTS = ["door", "pick"] as const;
export type CardButtonVariant = (typeof CARD_BUTTON_VARIANTS)[number];

export interface CardButtonProps
  extends Native, Omit<AriaButtonProps, "className" | "style" | "children" | "render"> {
  variant?: CardButtonVariant;
  /** Its name, the first thing on it. */
  title: ReactNode;
  /** What it does, under its title: its description. */
  line?: ReactNode;
  /** A mark before the title, in the accent: the door's icon. */
  mark?: ReactNode;
  /** What follows its title on its line, and in its name: a badge. */
  children?: ReactNode;
  /** A place for it in its view: margin, width, flex, order. Never a look. */
  className?: string;
  ref?: Ref<HTMLButtonElement>;
}

/** **A card pressed whole**: one button, named by its title and described by its line. */
export function CardButton({
  variant = "door",
  title,
  line,
  mark,
  children,
  className,
  ref,
  ...rest
}: CardButtonProps) {
  const titleId = useId();
  const moreId = useId();
  const lineId = useId();
  const { native, aria, own } = pressable(rest);
  // What follows the title, where anything does (`{project && <Badge />}` may draw nothing).
  const more = Children.toArray(children).length > 0;
  const named = rest["aria-label"] !== undefined || rest["aria-labelledby"] !== undefined;
  const describedBy = [own, line !== undefined ? lineId : undefined].filter(Boolean).join(" ");
  return (
    <AriaButton
      {...(aria as AriaButtonProps)}
      ref={ref}
      className={className}
      aria-labelledby={named ? rest["aria-labelledby"] : more ? `${titleId} ${moreId}` : titleId}
      aria-describedby={describedBy || undefined}
      render={renderPressed(native, undefined, own)}
      {...{ "data-scope": "card", "data-part": "pressable", "data-variant": variant }}
    >
      {mark !== undefined && (
        <span data-scope="card" data-part="mark" aria-hidden="true">
          {mark}
        </span>
      )}
      <span data-scope="card" data-part="title" id={titleId}>
        {title}
      </span>
      {more && (
        <span data-scope="card" data-part="more" id={moreId}>
          {children}
        </span>
      )}
      {line !== undefined && (
        <span data-scope="card" data-part="line" id={lineId}>
          {line}
        </span>
      )}
    </AriaButton>
  );
}
