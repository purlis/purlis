// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A card pressed whole** (#3655, its own family since #3885): a door on the welcome, a repo to
 * get.
 *
 * - **One button, the size of the card.** The card is a box (`root`); the button in it
 *   (`press`, React Aria's `Button` through the press every pressable family shares,
 *   `ui/pressable.tsx`) holds its title and what follows it, and its `::after` covers the whole
 *   card, so a press anywhere on it presses the button. It is pressed by the pointer, Enter and
 *   Space, keeps WebKit's tab sequence (#190) and draws its ring around the card, from the
 *   keyboard only.
 * - **Named by the words it shows** (WCAG 2.5.3, Label in Name): its title and what follows it
 *   (a badge), the button's own words. **Its line describes it**, from beside the button, so a
 *   screen reader reads the act first and the explanation after it, and the name is never more
 *   than what a person reads on the button.
 * - **Drawn by its parts**, a family of its own (`card-button`, #3885): its box
 *   `data-part="root"` with its `data-variant`, and `mark`, `press`, `title`, `more` and `line`.
 *   `card.css` draws them, and a theme keys them (`card-button.title.text`). It is not the card
 *   family's, so a theme that sets its cards flat on their panel (Purlis) never takes a pressed
 *   card's edge away, and a theme's `card.title` never reaches its title, even inside a `<Card>`;
 *   it is drawn in the `control.*` shades every theme sets, as a thing that is pressed.
 * - **Its box says the button's state** (`data-hovered`, `data-pressed`, `data-disabled`,
 *   `data-focus-visible`), as React Aria says it of the button, so the box's own rules draw it, a
 *   theme's state keys reach it, and nothing reads what the box holds.
 * - **Variants**: `door` (the default), a mark, a title and a line in a column on the control
 *   ground; `pick`, one line on the surface under it, a pick from a list.
 *   `row`, a mark beside its title and line with no edge of its own, one of a launcher's ways in;
 *   `current` says it is the one whose page is showing.
 * - **A place, never a look**: `className` gives the box a margin, a width or an order.
 */
import { Children, useId, useState, type ReactNode, type Ref } from "react";
import { Button as AriaButton, type ButtonProps as AriaButtonProps } from "react-aria-components";
import { mergeProps, useFocusRing } from "react-aria";
import { pressable, renderPressed, type Native } from "../pressable";

/** The pressable card's variants. */
export const CARD_BUTTON_VARIANTS = ["door", "pick", "row"] as const;
export type CardButtonVariant = (typeof CARD_BUTTON_VARIANTS)[number];

export interface CardButtonProps
  extends Native, Omit<AriaButtonProps, "className" | "style" | "children" | "render"> {
  variant?: CardButtonVariant;
  /** Its name, the first thing on it. */
  title: ReactNode;
  /** What it does, under its title: its description. */
  line?: ReactNode;
  /** Whether this card's page is the one showing: `aria-current` on its button. */
  current?: boolean;
  /** A mark before the title, in the accent: the door's icon. */
  mark?: ReactNode;
  /**
   * What follows its title on the button, and in its name: a badge. **Words and marks only,
   * never a link or another button**: it is drawn inside the card's one button, whose `::after`
   * covers the card, so anything interactive in it would be nested in a button (axe's
   * `nested-interactive`) and could never be pressed on its own.
   */
  children?: ReactNode;
  /** A place for it in its view: margin, width, flex, order. Never a look. */
  className?: string;
  /** The button's. */
  ref?: Ref<HTMLButtonElement>;
}

/** **A card pressed whole**: one button, named by its words and described by its line. */
export function CardButton({
  variant = "door",
  title,
  line,
  mark,
  current,
  children,
  className,
  ref,
  ...rest
}: CardButtonProps) {
  const lineId = useId();
  const [hovered, hover] = useState(false);
  const [pressed, press] = useState(false);
  const { focusProps, isFocusVisible } = useFocusRing();
  const { onHoverChange, onPressChange } = rest;
  const { native, aria, own } = pressable(rest);
  const off = rest.isDisabled === true || String(rest["aria-disabled"]) === "true";
  // What follows the title, where anything does (`{project && <Badge />}` may draw nothing).
  const more = Children.toArray(children).length > 0;
  const describedBy = [own, line !== undefined ? lineId : undefined].filter(Boolean).join(" ");
  return (
    <div
      className={className}
      data-scope="card-button"
      data-part="root"
      data-variant={variant}
      data-hovered={hovered && !off ? "" : undefined}
      data-pressed={pressed && !off ? "" : undefined}
      data-disabled={off ? "" : undefined}
      data-current={current ? "" : undefined}
      data-focus-visible={isFocusVisible ? "" : undefined}
    >
      {mark !== undefined && (
        <span data-scope="card-button" data-part="mark" aria-hidden="true">
          {mark}
        </span>
      )}
      <AriaButton
        {...(mergeProps(aria, focusProps) as AriaButtonProps)}
        ref={ref}
        onHoverChange={(now) => {
          hover(now);
          onHoverChange?.(now);
        }}
        onPressChange={(now) => {
          press(now);
          onPressChange?.(now);
        }}
        aria-describedby={describedBy || undefined}
        aria-current={current ? "page" : undefined}
        render={renderPressed(native, undefined, own)}
        {...{ "data-scope": "card-button", "data-part": "press" }}
      >
        <span data-scope="card-button" data-part="title">
          {title}
        </span>
        {more && (
          <>
            {" "}
            <span data-scope="card-button" data-part="more">
              {children}
            </span>
          </>
        )}
      </AriaButton>
      {line !== undefined && (
        <span data-scope="card-button" data-part="line" id={lineId}>
          {line}
        </span>
      )}
    </div>
  );
}
