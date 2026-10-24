// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";
import { symbolOf } from "./railIcons";
import { inForce, onDrawn, type RailSymbol } from "./theme";

/**
 * **The harness glyphs purlis draws** (#2922, ruling #2866 ·1): a spark for Claude Code, braces
 * for opencode and a terminal for Codex, in the stroke style of the rest of the icon set, never
 * a vendor's mark. On a 24 grid, stroked 2 wide with round caps and joins, as a rail icon is.
 *
 * They are the window's own, drawn where the theme in force draws none (Purlis Classic has no
 * icon set). The Purlis theme ships the same glyphs in its set (`icons/icons.json`), and an icon
 * pack redraws them there (·4): `purlis_core::theme::vocabulary::GLYPHS` names the same three.
 */
export const OWN_GLYPHS: Readonly<Record<string, RailSymbol>> = {
  spark: { paths: ["M12 2.7v18.6", "M2.7 12h18.6", "m5.4 5.4 13.2 13.2", "m18.6 5.4-13.2 13.2"] },
  braces: {
    paths: [
      "M8.25 4.5C5.7 4.5 6.9 10.5 3.9 12c3 1.5 1.8 7.5 4.35 7.5",
      "M15.75 4.5c2.55 0 1.35 6 4.35 7.5-3 1.5-1.8 7.5-4.35 7.5",
    ],
  },
  terminal: {
    paths: [
      "M6 3.9h12a3.3 3.3 0 0 1 3.3 3.3v9.6a3.3 3.3 0 0 1-3.3 3.3H6a3.3 3.3 0 0 1-3.3-3.3V7.2A3.3 3.3 0 0 1 6 3.9Z",
      "m6.9 9.6 3.3 2.4-3.3 2.4",
      "M12.9 15h4.2",
    ],
  },
};

/** No glyphs: what a theme without an icon set draws. */
const NONE: Readonly<Record<string, RailSymbol>> = {};

/** The glyphs of the theme in force; empty when it has none. */
function inForceGlyphs(): Readonly<Record<string, RailSymbol>> {
  return inForce().glyphs ?? NONE;
}

/** The glyph `name` as `theirs` draws it, else as the window does; nothing for a name neither
 *  has, which the caller draws as a monogram. */
export function glyphOf(
  theirs: Readonly<Record<string, RailSymbol>>,
  name: string,
): RailSymbol | undefined {
  return symbolOf(theirs, name) ?? symbolOf(OWN_GLYPHS, name);
}

/** **The harness glyphs the window draws**, the theme in force's, followed as it changes. */
export function useHarnessGlyphs(): (name: string) => RailSymbol | undefined {
  const theirs = useSyncExternalStore(onDrawn, inForceGlyphs, inForceGlyphs);
  return (name) => glyphOf(theirs, name);
}
