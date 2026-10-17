// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";
import { inForce, onDrawn, type RailSymbol } from "./theme";

/** No icon set: what a theme without one, Purlis Classic among them, draws the rails with. */
const NONE: Readonly<Record<string, RailSymbol>> = {};

/** The icon set of the theme in force, by view id; empty when it has none. */
function inForceIcons(): Readonly<Record<string, RailSymbol>> {
  return inForce().icons ?? NONE;
}

/** The symbol `set` draws `view` with, or nothing: only the set's own views, never a name an
 *  object inherits. */
export function symbolOf(
  set: Readonly<Record<string, RailSymbol>>,
  view: string,
): RailSymbol | undefined {
  return Object.prototype.hasOwnProperty.call(set, view) ? set[view] : undefined;
}

/**
 * **The icon the rails draw a view with** (#2337): the theme in force's, followed as it
 * changes, or nothing where the theme draws no icon for the view and the window's own mark
 * stands. The core's theme engine resolved the set with the theme; the rails only draw it.
 */
export function useRailIcons(): (view: string) => RailSymbol | undefined {
  const set = useSyncExternalStore(onDrawn, inForceIcons, inForceIcons);
  return (view) => symbolOf(set, view);
}
