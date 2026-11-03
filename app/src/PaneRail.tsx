// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Hand } from "lucide-react";
import { useCallback, useLayoutEffect, useState, type ReactNode } from "react";
import { HOLDS_NOTHING, type BandHolds } from "./paneNotices";
import type { Size } from "./paneDrop";
import { Button } from "./ui/button/Button";

/**
 * **A folded side of a split** (#4766, spec #4750 stories 81 to 83; `prototype/narrow-window`
 * Q3 A): a 40px rail beside the pane that kept the room, or a 32px bar above or below it. One
 * button per folded pane, each with the pane's icon (and its state, where the icon wears it),
 * its notes count and ✋ while it asks, so no pane goes off screen and no ask hides. A press
 * opens that pane: it takes the focus, and the side that held it folds instead
 * (`paneFold.ts`). A bar has room for each pane's name; a rail says it in its tooltip.
 */

/** One folded pane, as its rail draws it. */
export type RailPane = {
  pane: number;
  /** What the pane holds, by name: its chat, its view, or what an empty pane would do. */
  name: string;
  /** Its icon: a chat's persona and state, a view's mark. */
  mark: ReactNode;
  /** The notes its band held when it was last drawn (`keptHolds`). */
  holds: BandHolds;
  /** The chat it shows asks the person something now. */
  asks: boolean;
};

/**
 * **A folded pane, as one button**: named by its own words, never an `aria-label` over them, so
 * what is seen is what is heard (WCAG 2.5.3): its name (drawn on a bar, heard on a rail), its
 * notes count, drawn, with "notices" heard after it, then "folded" and "asks you", heard. Each
 * button is at least a control tall and the rail's width, a target the pointer can't miss.
 */
export function PaneRail({
  as,
  panes,
  onOpen,
}: {
  as: "rail" | "bar";
  panes: readonly RailPane[];
  onOpen: (pane: number) => void;
}) {
  return (
    <div className="pane-rail" data-fold={as} role="group" aria-label="Folded panes">
      {panes.map((one) => {
        const notes = one.holds.notes;
        return (
          <Button
            key={one.pane}
            variant="quiet"
            size="sm"
            className="pane-rail-pane"
            data-pane={one.pane}
            tooltip={`Open ${one.name}`}
            icon={one.mark}
            onPress={() => onOpen(one.pane)}
          >
            {/* One run of words, so the button's flex box makes no item (no block, which a name
                computation spaces either side of) of any one of them: each comma is inside the
                word it follows, and each space is text between words. */}
            <span className="pane-rail-words">
              <span className={as === "bar" ? "pane-rail-name" : "hidden-words"}>
                {one.name}
                <span className="pane-rail-pause">,</span>
              </span>{" "}
              {notes > 0 && (
                <>
                  <span
                    className="pane-rail-count"
                    data-trouble={one.holds.trouble > 0 ? "" : undefined}
                  >
                    {notes}
                  </span>{" "}
                  <span className="hidden-words">{notes === 1 ? "notice," : "notices,"}</span>{" "}
                </>
              )}
              <span className="hidden-words">{one.asks ? "folded, asks you" : "folded"}</span>
            </span>
            {one.asks && (
              <Hand className="pane-rail-asks" data-mark="needs-you" aria-hidden="true" />
            )}
          </Button>
        );
      })}
    </div>
  );
}

/**
 * **What each pane's band held when it was last drawn**, by its band (`paneBand`): a folded
 * pane's notices are not drawn, so its rail says the count its bell last said. Written by the
 * pane's frame whenever its band changes.
 */
const held = new Map<string, BandHolds>();

export function keepHolds(band: string, holds: BandHolds): void {
  held.set(band, holds);
}

export function keptHolds(band: string): BandHolds {
  return held.get(band) ?? HOLDS_NOTHING;
}

/**
 * **The room an element gives the panes**, as it is resized: nothing until it has been laid
 * out with some width and height (jsdom never lays out), so nothing folds before then.
 */
export function usePaneRoom(): [(element: HTMLElement | null) => void, Size | undefined] {
  const [element, setElement] = useState<HTMLElement | null>(null);
  const [room, setRoom] = useState<Size | undefined>(undefined);
  useLayoutEffect(() => {
    if (element === null) return;
    const measure = () => {
      const box = element.getBoundingClientRect();
      const next =
        box.width > 0 && box.height > 0
          ? { width: Math.round(box.width), height: Math.round(box.height) }
          : undefined;
      setRoom((was) => (was?.width === next?.width && was?.height === next?.height ? was : next));
    };
    measure();
    const watching = new ResizeObserver(measure);
    watching.observe(element);
    return () => watching.disconnect();
  }, [element]);
  const at = useCallback((one: HTMLElement | null) => setElement(one), []);
  return [at, room];
}
