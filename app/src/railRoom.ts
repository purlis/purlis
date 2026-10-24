// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useLayoutEffect, useSyncExternalStore } from "react";
import { useProjectShown } from "./projectShown";
import { DEFAULT_TEXT, textSizes, useTextSizes } from "./textSize";

/**
 * **The project rail never takes the room the work was held to** (#3077, ADR 0054's amendment
 * of 2026-10-24).
 *
 * The views below the title bar are held to fit two windows: the narrowest one, 1024 px (ADR
 * 0054), and, with both sidebars open, the default one, 1280 px (`tauri.conf.json`; DS-9's
 * tables, the settings row, the merge list). The rail is 52 px of the window. In a window where
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
 *  held to fit: the narrowest window, and the default one with both sidebars open. */
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

/** The sidebars the project in front draws, as its frame last said: both, as the default
 *  arrangement does, until a frame has said. A frame says before the window first paints. */
const BOTH = 2;
let sides = BOTH;
const sidesTold = new Set<() => void>();
const sidesNow = (): number => sides;
const onSides = (draw: () => void) => {
  sidesTold.add(draw);
  return () => void sidesTold.delete(draw);
};

/**
 * **The frame of the project in front says how many sidebars it draws** (`RegionFrame`), before
 * the window paints, so the rail is drawn for the arrangement on screen. A project kept behind
 * another says nothing.
 */
export function useSidesOfTheFront(open: number): void {
  const shown = useProjectShown();
  useLayoutEffect(() => {
    if (!shown || sides === open) return;
    sides = open;
    for (const draw of sidesTold) draw();
  }, [shown, open]);
}

const windowWidth = () => window.innerWidth;
const onWindowWidth = (draw: () => void) => {
  window.addEventListener("resize", draw);
  return () => window.removeEventListener("resize", draw);
};

/** Whether the rail crowds the work now, for what is not drawn: a press's handler. */
export function railCrowdedNow(): boolean {
  return railCrowds(windowWidth(), sidesNow(), textSizes().window);
}

/** Whether the rail crowds the work, following the window's width, its text size and the
 *  sidebars of the project in front. */
export function useRailCrowded(): boolean {
  const width = useSyncExternalStore(onWindowWidth, windowWidth);
  const open = useSyncExternalStore(onSides, sidesNow);
  const text = useTextSizes().window;
  return railCrowds(width, open, text);
}

/** Forgets the sidebars a frame said, as a new launch would. For tests. */
export function forgetTheSides(): void {
  sides = BOTH;
}
