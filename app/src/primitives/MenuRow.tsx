// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { flushSync } from "react-dom";
import * as Tooltip from "@radix-ui/react-tooltip";

/**
 * **What is inside a menu's row** (`docs/ui-primitives.md`, "Menus"): its words, the one line
 * that may stand under them, the key or mark at its end, and its tooltip. The row itself is
 * Radix's `Item`, `CheckboxItem` or `SubTrigger`, a context menu's or a dropdown's, wearing
 * `ui-menu-item`; these are what it holds, so every menu in the window is drawn alike.
 */

/**
 * How long the keyboard or the pointer rests on a row before its tooltip is shown: twice the
 * window's tooltips (`StatusLine.tsx`), because a pointer crosses a menu's rows on its way to
 * the one it wants and a tooltip for each row it crossed would be the noise the tooltip
 * replaced.
 */
export const TIP_AFTER_MS = 600;

/**
 * Whether the keyboard or the pointer has **rested** on a menu's row, and the two handlers that
 * tell it. Radix gives a row the keyboard when the pointer comes onto it, so its focus is where
 * the pointer is as well as where the arrows are, and one pair of handlers reads both.
 *
 * `rested` goes false when the row loses the keyboard, and `wake` makes it false at once: a
 * press, or Escape.
 */
export function useRested(has: boolean): {
  rested: boolean;
  rest: { onFocus: () => void; onBlur: () => void };
  wake: () => void;
} {
  const [rested, setRested] = useState(false);
  const soon = useRef(0);
  const wake = useCallback(() => {
    window.clearTimeout(soon.current);
    soon.current = 0;
    setRested(false);
  }, []);
  const onFocus = useCallback(() => {
    if (!has) return;
    window.clearTimeout(soon.current);
    soon.current = window.setTimeout(() => setRested(true), TIP_AFTER_MS);
  }, [has]);
  useEffect(() => () => window.clearTimeout(soon.current), []);
  return { rested: has && rested, rest: { onFocus, onBlur: wake }, wake };
}

/**
 * A row's tooltip: what the row does that its words do not say, beside the menu, while the
 * keyboard or the pointer rests on it ({@link useRested}). Drawn inside the row, and anchored
 * to the whole of it by a part that draws nothing.
 *
 * The window's tooltip (Radix Tooltip, `.ui-tooltip`), opened by the row's rest and not by
 * Radix's own hover: the pointer coming onto a row gives it the keyboard, and Radix shows a
 * tooltip at once for a focus.
 *
 * **Escape closes the menu, not only the tooltip.** A Radix tooltip is the top layer while it
 * is up and would take the first Escape for itself. It is taken down on the window's capture
 * phase, before Radix's own listener on the document, so the same key reaches the menu.
 */
export function MenuTip({
  side,
  onClose,
  children,
}: {
  /** Which side of the row it stands on: away from a submenu the row opens. */
  side: "left" | "right";
  onClose: () => void;
  children: ReactNode;
}) {
  useEffect(() => {
    const down = (event: KeyboardEvent) => {
      if (event.key === "Escape") flushSync(onClose);
    };
    window.addEventListener("keydown", down, true);
    return () => window.removeEventListener("keydown", down, true);
  }, [onClose]);
  return (
    <Tooltip.Provider>
      <Tooltip.Root
        open
        onOpenChange={(next) => {
          if (!next) onClose();
        }}
      >
        <Tooltip.Trigger asChild>
          <span className="ui-menu-tip-anchor" aria-hidden="true" />
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content
            className="ui-tooltip ui-menu-tip ui-text-secondary"
            side={side}
            align="start"
            sideOffset={8}
            collisionPadding={8}
            // The row is described by its own hidden line, which is there before the rest.
            aria-hidden="true"
          >
            {children}
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
    </Tooltip.Provider>
  );
}

/**
 * A row's words, and what may stand under them: what a row that cannot be undone costs, or why
 * a row cannot run. The words are one line and give way to an ellipsis. **What is under them is
 * never cut**: a warning that lost its last words, or a reason a keyboard cannot open, would
 * say less than nothing, so it wraps.
 */
export function MenuWords({
  said,
  under,
  danger,
}: {
  said: string;
  under?: string;
  /** In the danger colour, which the row's own class sets: the words wear no role then. */
  danger?: boolean;
}) {
  return (
    <span className="ui-menu-words">
      <span className={danger ? "ui-menu-title" : "ui-menu-title ui-text-body"}>{said}</span>
      {under !== undefined && under !== "" && (
        <span className="ui-menu-note ui-text-secondary">{under}</span>
      )}
    </span>
  );
}

/** The key that does what the row does, at the row's end. */
export function MenuKeys({ children }: { children: string }) {
  return <span className="ui-menu-keys ui-text-meta">{children}</span>;
}
