// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { flushSync } from "react-dom";
import { AnchoredTooltip, onTooltipEscape, TOOLTIP_CLOSE_DELAY } from "../tooltip/Tooltip";

/**
 * **A menu row's tooltip** (`docs/ui-primitives.md`, "Menus", rule 2): what the row does that its
 * words do not say, beside the menu, once the keyboard or the pointer has rested on the row.
 * The row itself is the family's `MenuItem`; its words, its key and its mark are drawn by it.
 */

/**
 * How long the keyboard or the pointer rests on a row before its tooltip is shown: twice the
 * window's tooltips (`StatusLine.tsx`), because a pointer crosses a menu's rows on its way to
 * the one it wants and a tooltip for each row it crossed would be the noise the tooltip
 * replaced.
 */
export const TIP_AFTER_MS = 600;

/** Every key the window has heard, counted on its capture phase once a menu row is drawn. */
let keysHeard = 0;
let listening = false;
function listenForKeys() {
  if (listening) return;
  listening = true;
  window.addEventListener("keydown", () => (keysHeard += 1), { capture: true });
}

/**
 * Whether the keyboard or the pointer has **rested** on a menu's row, and the two handlers that
 * tell it. React Aria gives a row the keyboard when the pointer comes onto it, so its focus is
 * where the pointer is as well as where the arrows are, and one pair of handlers reads both.
 *
 * `rested` goes false a moment after the row loses the keyboard, unless the pointer has gone
 * onto the row's tooltip to read it (`hold`, WCAG 1.4.13 hoverable), and `wake` makes it false
 * at once: a press, or Escape.
 */
export function useRested(has: boolean): {
  rested: boolean;
  rest: { onFocus: (event?: unknown) => void; onBlur: () => void };
  wake: () => void;
  hold: { onPointerEnter: () => void; onPointerLeave: () => void };
} {
  const [rested, setRested] = useState(false);
  const soon = useRef(0);
  const later = useRef(0);
  const held = useRef(false);
  // How many keys the window had heard when the row was drawn.
  const keysAtFirst = useRef(-1);
  useEffect(() => {
    listenForKeys();
    keysAtFirst.current = keysHeard;
  }, []);
  const wake = useCallback(() => {
    window.clearTimeout(soon.current);
    window.clearTimeout(later.current);
    soon.current = 0;
    held.current = false;
    setRested(false);
  }, []);
  const onFocus = useCallback(
    (event?: unknown) => {
      if (!has) return;
      // **Only a person's rest.** A menu puts the keyboard on its first row by itself as it
      // opens; that is no rest, and a tooltip it raised would take the Escape meant for the
      // menu. A row the pointer is on, or one a key moved to after the menu came, is.
      const row = (event as { currentTarget?: unknown } | undefined)?.currentTarget;
      const hovered = row instanceof Element && row.matches(":hover");
      if (!hovered && keysHeard === keysAtFirst.current) return;
      window.clearTimeout(soon.current);
      window.clearTimeout(later.current);
      soon.current = window.setTimeout(() => setRested(true), TIP_AFTER_MS);
    },
    [has],
  );
  // Gone from the row, the tooltip stays for the pointer to cross onto it, and goes unless it did.
  const onBlur = useCallback(() => {
    window.clearTimeout(soon.current);
    window.clearTimeout(later.current);
    later.current = window.setTimeout(() => {
      if (!held.current) setRested(false);
    }, TOOLTIP_CLOSE_DELAY);
  }, []);
  const hold = useMemo(
    () => ({
      onPointerEnter: () => {
        held.current = true;
        window.clearTimeout(later.current);
      },
      onPointerLeave: () => {
        held.current = false;
        later.current = window.setTimeout(() => setRested(false), TOOLTIP_CLOSE_DELAY);
      },
    }),
    [],
  );
  useEffect(
    () => () => {
      window.clearTimeout(soon.current);
      window.clearTimeout(later.current);
    },
    [],
  );
  return { rested: has && rested, rest: { onFocus, onBlur }, wake, hold };
}

/**
 * A row's tooltip: what the row does that its words do not say, beside the menu, while the
 * keyboard or the pointer rests on it ({@link useRested}), or on the tooltip itself. Drawn
 * inside the row, and anchored to the whole of it by a part that draws nothing.
 *
 * The window's shared tooltip (`ui/tooltip`, #2908), opened by the row's rest and not by a
 * pointer on it: the pointer coming onto a row gives it the keyboard, and a tooltip shown at
 * once for each row a pointer crossed would be noise.
 *
 * **Escape closes the tooltip, and only the tooltip** (WCAG 1.4.13): it is heard on the window's
 * capture phase, before the menu's own listener on the document, and goes no further, so the
 * menu stays open with the keyboard on its row. The next Escape is the menu's.
 */
export function MenuTip({
  side,
  onClose,
  hold,
  children,
}: {
  /** Which side of the row it stands on: away from a submenu the row opens. */
  side: "left" | "right";
  onClose: () => void;
  /** The pointer on the tooltip keeps it ({@link useRested}). */
  hold?: { onPointerEnter: () => void; onPointerLeave: () => void };
  children: ReactNode;
}) {
  const anchor = useRef<HTMLSpanElement>(null);
  useEffect(() => onTooltipEscape(() => flushSync(onClose)), [onClose]);
  return (
    <>
      <span ref={anchor} data-scope="menu" data-part="tip-anchor" aria-hidden="true" />
      <AnchoredTooltip
        anchor={anchor}
        side={side === "right" ? "end" : "start"}
        onPointerEnter={hold?.onPointerEnter}
        onPointerLeave={hold?.onPointerLeave}
      >
        <span data-scope="menu" data-part="tip" className="ui-text-secondary">
          {children}
        </span>
      </AnchoredTooltip>
    </>
  );
}
