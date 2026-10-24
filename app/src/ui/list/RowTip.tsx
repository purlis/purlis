// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A row's tooltip** (#2911): what a row of a list, a tree or a table says that its words do
 * not (the whole of words cut to a line, the date a memory was written), beside the row, as the
 * window's shared tooltip (`ui/tooltip`, #2908). A row is drawn by its collection, so no
 * `Tooltip` can wrap it; the row shows its own instead:
 *
 * - **under the pointer**, after the window's tooltip delay, until the pointer leaves;
 * - **at once while the keyboard is on the row** (its focus ring is showing), until it leaves;
 * - **Esc takes it down** without moving the keyboard, and is the tooltip's alone (WCAG 1.4.13).
 *
 * It is anchored to the whole row by a part that draws nothing and takes no pointer.
 */
import { useCallback, useContext, useEffect, useId, useRef, useState, type ReactNode } from "react";
import { flushSync } from "react-dom";
import { AnchoredTooltip, onTooltipEscape, TOOLTIP_DELAY, type Tip } from "../tooltip/Tooltip";
import { RowScope } from "./Row";

/** What a row with a tooltip hands its element, and draws inside it. */
export interface RowTipped {
  /** The pointer's handlers for the row: its own, so a row that runs nothing shows one too. */
  hover: {
    onPointerEnter?: (event: React.PointerEvent) => void;
    onPointerLeave?: (event: React.PointerEvent) => void;
  };
  /** What the row draws inside itself: the anchor, and the tooltip while it is up. */
  drawn: (keyboardOn: boolean) => ReactNode;
}

/** The handlers and the drawing of a row's tooltip; nothing at all for a row without one. */
export function useRowTip(tip: Tip | undefined): RowTipped {
  const scope = useContext(RowScope);
  const anchor = useRef<HTMLSpanElement>(null);
  const [pointed, setPointed] = useState(false);
  const [dismissed, setDismissed] = useState(false);
  const soon = useRef(0);
  useEffect(() => () => window.clearTimeout(soon.current), []);
  const onPointerEnter = useCallback((event: React.PointerEvent) => {
    if (event.pointerType === "touch") return;
    window.clearTimeout(soon.current);
    soon.current = window.setTimeout(() => setPointed(true), TOOLTIP_DELAY);
  }, []);
  const onPointerLeave = useCallback(() => {
    window.clearTimeout(soon.current);
    setPointed(false);
    setDismissed(false);
  }, []);
  if (tip === undefined || tip === null || tip === "") return { hover: {}, drawn: () => null };
  return {
    hover: { onPointerEnter, onPointerLeave },
    drawn: (keyboardOn) => (
      <>
        <span ref={anchor} data-scope={scope} data-part="tip-anchor" aria-hidden="true" />
        {(pointed || keyboardOn) && !dismissed && (
          <Shown anchor={anchor} onEscape={() => setDismissed(true)}>
            {tip}
          </Shown>
        )}
      </>
    ),
  };
}

/** The tooltip while it is up, which the first Esc takes down. */
function Shown({
  anchor,
  onEscape,
  children,
}: {
  anchor: React.RefObject<HTMLSpanElement | null>;
  onEscape: () => void;
  children: Tip;
}) {
  const scope = useContext(RowScope);
  useEffect(() => onTooltipEscape(() => flushSync(onEscape)), [onEscape]);
  // **The tooltip is the row's description while it is shown**, as every tooltip is its
  // target's: the row is drawn by its collection, so it is said on the row's element here.
  const fallback = useId();
  const tooltip = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const row = anchor.current?.closest(`[data-scope="${scope}"][data-part="root"]`);
    const shown = tooltip.current;
    if (!row || !shown) return;
    if (shown.id === "") shown.id = fallback;
    const id = shown.id;
    const was = row.getAttribute("aria-describedby");
    row.setAttribute("aria-describedby", was ? `${was} ${id}` : id);
    return () => {
      const now = (row.getAttribute("aria-describedby") ?? "")
        .split(/\s+/)
        .filter((one) => one !== "" && one !== id);
      if (now.length === 0) row.removeAttribute("aria-describedby");
      else row.setAttribute("aria-describedby", now.join(" "));
    };
  }, [anchor, fallback, scope]);
  return (
    <AnchoredTooltip anchor={anchor} tooltipRef={tooltip}>
      {children}
    </AnchoredTooltip>
  );
}
