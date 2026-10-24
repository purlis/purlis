// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useLayoutEffect, useRef, useSyncExternalStore } from "react";
import { useProjectShown } from "./projectShown";
import { DEFAULT_TEXT, onTextSizes, textSizes } from "./textSize";

/**
 * **The project rail never takes the room the work was held to** (#3077, ADR 0054's amendment
 * of 2026-10-24).
 *
 * The views below the title bar are held to fit two windows: the narrowest one, 1024 px (ADR
 * 0054), and, with both sidebars open, a 1280 px one (DS-9's tables, the settings row, the merge
 * list; the default window until it grew to 1440 px, ADR 0054's second amendment of 2026-10-24).
 * The rail is 52 px of the window. In a window where
 * those 52 px would leave the work less than it was held to, the rail hides itself (it peeks at
 * the window's left edge as it does on auto-hide), and it comes back when the window is wide
 * enough again or a sidebar is put away.
 *
 * **What is drawn, never what is kept**, as `narrowPanel.ts` draws one half of the Panel: the
 * rail's own mode (`railMode.ts`, the layout file's `rail`) is the person's, and a hide the
 * window makes is never written there.
 *
 * **Counted at the default text size**, as the narrow Panel is: every size in the window is in
 * `rem`, so a bigger window text crowds the work as a narrower window would. The rail's own
 * 52 px are pixels, so they come off the window before it is counted.
 */

/** The rail's column, in CSS pixels (`ui/rail/rail.css`, R76). */
export const RAIL_WIDTH = 52;

/** The window widths, in CSS pixels at the default text size, the work below the title bar is
 *  held to fit: the narrowest window, and a 1280 px one with both sidebars open. */
export const ROOM_HELD = { narrowest: 1024, bothSides: 1280 } as const;

/**
 * Whether the rail, drawn in a window `width` CSS pixels wide with `sides` sidebars open and its
 * text at `text` px, would leave the work less room than it is held to.
 */
export function railCrowds(
  width: number,
  sides: number,
  text: number = DEFAULT_TEXT.window,
): boolean {
  const held = sides >= 2 ? ROOM_HELD.bothSides : ROOM_HELD.narrowest;
  return ((width - RAIL_WIDTH) * DEFAULT_TEXT.window) / text < held;
}

/** The sidebars the project in front draws, as its frame says: both, as the default arrangement
 *  does, while no frame says (none has yet, or the frame that said has gone, its project closed
 *  or sent behind). A frame says before the window first paints. */
const BOTH = 2;
let sides = BOTH;
/** The frame whose count {@link sides} is. */
let saidBy: object | undefined;
const told = new Set<() => void>();
const tell = () => {
  for (const draw of told) draw();
};
const onCrowding = (draw: () => void) => {
  told.add(draw);
  window.addEventListener("resize", draw);
  const stop = onTextSizes(draw);
  return () => {
    told.delete(draw);
    window.removeEventListener("resize", draw);
    stop();
  };
};

/**
 * **The frame of the project in front says how many sidebars it draws** (`RegionFrame`), before
 * the window paints, so the rail is drawn for the arrangement on screen. A project kept behind
 * another says nothing, and a frame that goes takes what it said with it.
 */
export function useSidesOfTheFront(open: number): void {
  const shown = useProjectShown();
  const frame = useRef({});
  useLayoutEffect(() => {
    if (!shown) return;
    const me = frame.current;
    saidBy = me;
    if (sides !== open) {
      sides = open;
      tell();
    }
    return () => {
      if (saidBy !== me) return;
      saidBy = undefined;
      if (sides !== BOTH) {
        sides = BOTH;
        tell();
      }
    };
  }, [shown, open]);
}

/** Whether the rail crowds the work now: the window's width, its text size and the sidebars of
 *  the project in front. */
export function railCrowdedNow(): boolean {
  return railCrowds(window.innerWidth, sides, textSizes().window);
}

/**
 * {@link railCrowdedNow}, for a component that redraws when it changes: only when it changes,
 * not on every pixel of a window being dragged wider.
 */
export function useRailCrowded(): boolean {
  return useSyncExternalStore(onCrowding, railCrowdedNow);
}

/** Forgets the sidebars a frame said, as a new launch would. For tests. */
export function forgetTheSides(): void {
  sides = BOTH;
  saidBy = undefined;
}
