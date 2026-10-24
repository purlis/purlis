// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { layoutPref } from "./layoutPref";

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

/** The store (`layoutPref.ts`), under `rail`. */
export const RAIL_MODE = layoutPref<RailMode, "rail">({
  key: "rail",
  fallback: "shown",
  load: loadRailMode,
  same: (one, other) => one === other,
  written: (mode) => (mode === "auto" ? "auto" : undefined),
  remedy: (where) =>
    `fix ${where}, or show or hide the rail from its own button, which rewrites it`,
});

/** The rail's mode, for a component that redraws when it changes, and the way to change it. */
export function useRailMode(): [RailMode, (mode: RailMode) => void] {
  return [RAIL_MODE.use(), RAIL_MODE.set];
}

/** Forgets what this launch chose and read, as a new launch would. For tests. */
export function forgetRailMode(): void {
  RAIL_MODE.forget();
}
