// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";
import { layoutPref } from "./layoutPref";
import { forgetTheSides, railCrowdedNow, useRailCrowded } from "./railRoom";

/**
 * **Whether the project rail is always shown or hides itself** (#2915, R82, R83): `shown` keeps
 * it in its column; `auto` hides it, and it peeks from the window's left edge under the pointer
 * (R86) while the title bar's first button shows it again (R91).
 *
 * **Kept in the layout file under `rail`** (`charter/layout.json`, `regions.ts`), beside the
 * window's other preferences on this machine, and read from what the window was handed as it was
 * created, so the first frame draws the rail the way it was left and never slides it into place.
 * Only `auto` is written: always shown is the default, which the file leaves out. The rail's own
 * buttons are the only places it is changed (#2868·2).
 *
 * **What the window draws can be hidden where the mode is shown** (#3077, `railRoom.ts`): in a
 * window where the rail would crowd the work, it hides itself, and that hide is drawn over the
 * mode and never written. It never overrides the person: a rail they hid stays hidden in a wide
 * window, and a rail they show in a narrow one stays shown ({@link railDrawn}).
 */
export type RailMode = "shown" | "auto";

/** The rail's mode as a layout document holds it, and what had to be put right. */
export function loadRailMode(raw: unknown): { value: RailMode; said: string[] } {
  const held =
    raw !== null && typeof raw === "object" && !Array.isArray(raw)
      ? (raw as { rail?: unknown }).rail
      : undefined;
  if (held === undefined || held === "shown") return { value: "shown", said: [] };
  if (held === "auto") return { value: "auto", said: [] };
  return {
    value: "shown",
    said: [`"rail" ${JSON.stringify(held)} is not "shown" or "auto", so the rail is shown`],
  };
}

/**
 * **The person showed the rail in a window that crowds it**: the title bar's show, or Keep
 * sidebar shown from the peek, pressed while the window hid the rail. The window's hide gives way
 * to it for the rest of this launch, however often the window narrows again, until the person
 * hides the rail. Known to this launch only: the layout file keeps the mode, and no more.
 */
let shownAnyway = false;
const shownAnywayTold = new Set<() => void>();
const onShownAnyway = (draw: () => void) => {
  shownAnywayTold.add(draw);
  return () => void shownAnywayTold.delete(draw);
};

/** The store (`layoutPref.ts`), under `rail`. */
export const RAIL_MODE = layoutPref<RailMode, "rail">({
  key: "rail",
  fallback: "shown",
  load: loadRailMode,
  same: (one, other) => one === other,
  written: (mode) => (mode === "auto" ? "auto" : undefined),
  remedy: (where) =>
    `fix ${where}, or show or hide the rail from its own button, which rewrites it`,
  forgotten: () => {
    shownAnyway = false;
    forgetTheSides();
  },
});

/**
 * **How the rail is drawn**: hidden where the person hid it; else hidden where it would crowd the
 * work (`crowded`), unless the person showed it there (`anyway`); else shown.
 */
export function railDrawn(mode: RailMode, crowded: boolean, anyway: boolean): RailMode {
  if (mode === "auto") return "auto";
  return crowded && !anyway ? "auto" : "shown";
}

/**
 * **What a press of the person's leaves of their showing it anyway**: a hide ends it; a show in a
 * window that crowds the rail starts it; a show elsewhere leaves it as it was.
 */
export function shownAnywayAfter(was: boolean, to: RailMode, crowded: boolean): boolean {
  return to === "auto" ? false : was || crowded;
}

/** The person shows or hides the rail: the mode, written where it changed, and whether the
 *  window's hide gives way. */
function railChosen(to: RailMode): void {
  const next = shownAnywayAfter(shownAnyway, to, railCrowdedNow());
  if (next !== shownAnyway) {
    shownAnyway = next;
    for (const draw of shownAnywayTold) draw();
  }
  RAIL_MODE.set(to);
}

/**
 * The rail as the window draws it ({@link railDrawn}), for a component that redraws when it
 * changes, and the way for the person to change it.
 */
export function useRailMode(): [RailMode, (mode: RailMode) => void] {
  const mode = RAIL_MODE.use();
  const crowded = useRailCrowded();
  const anyway = useSyncExternalStore(onShownAnyway, () => shownAnyway);
  return [railDrawn(mode, crowded, anyway), railChosen];
}

/** Forgets what this launch chose and read, as a new launch would. For tests. */
export function forgetRailMode(): void {
  RAIL_MODE.forget();
}
