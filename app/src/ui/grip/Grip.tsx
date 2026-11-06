// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The grip family** (#5290, spec #5288, ADR 0104): what a pane or a chat is picked up by, one
 * component in a chat's row in the list and a pane's head, so the two cannot drift apart. A
 * screen imports this and never draws its own (`purlis/grip-family` in `lint.js`).
 *
 * - **It is the mark it stands for, until it is reached.** At rest it draws its `children` (a
 *   persona's mark, a shell's, a view's); under the pointer, or with the keyboard on it or on the
 *   row it stands in, six dots in three rows of two take the mark's place, in the same box, so
 *   nothing moves. Only the grip under the pointer changes, so a list keeps its marks as the
 *   pointer runs down it. It stands in a chat mark's `who` (`ui/chat-mark`), so the state's badge
 *   on the corner stays drawn.
 * - **No button ground**, in any state: no fill, edge or shadow (`grip.css`; the controls guard
 *   holds it, and the theme's root has nothing to set). The cursor is an open hand, closed while
 *   it is pressed and while a drag holds it.
 * - **A control of its own, or its row's.** Given a `label` ("Move" and what it moves) it is a
 *   button: Tab reaches it, and a press, Enter or Space picks up what it moves, which is the
 *   single press WCAG 2.5.7 asks for in place of a drag. With none it stands inside a control
 *   that is pressable itself (a chat's row, a button): it is no control and is not heard, its
 *   press picks up and goes no further, and the keyboard on the row shows its dots.
 * - **Drawn by its parts**: `root`, `mark`, `dots` and each `dot`, whose fill a theme reaches
 *   (`grip.dot.hover.fill`, `grip.dot.focused.fill`; ADR 0103).
 */
import type { ReactNode, Ref } from "react";
import { Button as AriaButton } from "react-aria-components";

/** **How many dots, in rows of two** (the person's ask on the prototype, 3×2). */
const DOTS = 6;

export type GripProps = {
  /** The mark it stands for, drawn at rest. */
  children: ReactNode;
  /** Picks up what it moves: a press, or Enter or Space on a grip that is a control. */
  onPress: () => void;
  /** Its name, "Move" and what it moves, which makes it a control of its own (a pane's head).
   *  Left out inside a control that is pressable itself (a chat's row). */
  label?: string;
  /** The grip as drawn, where the keyboard goes back to it. */
  gripRef?: Ref<HTMLElement>;
  /** Marks a screen puts on it so a drag or a region knows it (`data-*`), never a look. */
  marks?: Readonly<Record<`data-${string}`, string>>;
};

/** **A grip**: its mark at rest, dots to be picked up by. */
export function Grip({ children, onPress, label, gripRef, marks }: GripProps) {
  const inside = (
    <>
      <span data-scope="grip" data-part="mark">
        {children}
      </span>
      <span data-scope="grip" data-part="dots" aria-hidden="true">
        {Array.from({ length: DOTS }, (_, at) => (
          <span key={at} data-scope="grip" data-part="dot" />
        ))}
      </span>
    </>
  );
  if (label !== undefined)
    return (
      <AriaButton
        {...marks}
        ref={gripRef as Ref<HTMLButtonElement>}
        data-scope="grip"
        data-part="root"
        aria-label={label}
        onPress={onPress}
      >
        {inside}
      </AriaButton>
    );
  return (
    <span
      {...marks}
      ref={gripRef as Ref<HTMLSpanElement>}
      data-scope="grip"
      data-part="root"
      // A drag from it is its row's drag; its press picks up, and goes no further than it.
      onClick={(event) => {
        event.stopPropagation();
        onPress();
      }}
    >
      {inside}
    </span>
  );
}
