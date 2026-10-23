// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useSyncExternalStore } from "react";
import { inForce, onDrawn, tintVariables, type Theme, type Token } from "../../theme/theme";

/** The tokens a project's button puts on itself in its colour: its monogram and its ground. */
export const PROJECT_TOKENS: readonly Token[] = ["accent.base", "accent.surface"];

/**
 * **A project's own colour on the rail** (R77): a colour of the theme's own palette, the one the
 * project's name falls on. A project has no colour of its own to name, so its name picks one, the
 * same one on every launch and in every window, and the theme decides what that hue looks like.
 * `undefined` in a theme whose palette has no hues, which draws every project in the accent.
 */
export function projectColour(theme: Theme, name: string): string | undefined {
  const hues = theme.tint.hues;
  return hues.length === 0 ? undefined : hues[fnv1a(name) % hues.length].name;
}

/**
 * **What a project's button puts on itself** (`rail.css`): the accent and its surface turned to
 * the project's colour, as a persona's mark puts its own on itself. The luminance of each is kept
 * exactly (`tint.ts`), so the monogram clears on its ground every contrast the accent clears on
 * its surface, whatever the hue. Empty where the theme gives no colour.
 */
export function projectTint(theme: Theme, name: string): Record<string, string> {
  return tintVariables(theme, projectColour(theme, name), PROJECT_TOKENS);
}

/** A project's colour and {@link projectTint}, by its name, in the theme in force, followed as
 *  it changes. */
export function useProjectTints(): (name: string) => {
  colour: string | undefined;
  tint: Record<string, string>;
} {
  const theme = useSyncExternalStore(onDrawn, inForce, inForce);
  return useCallback(
    (name: string) => ({ colour: projectColour(theme, name), tint: projectTint(theme, name) }),
    [theme],
  );
}

/** FNV-1a over the name's code points: a small, stable spread, the same everywhere. */
function fnv1a(text: string): number {
  let hash = 0x811c9dc5;
  for (const char of text) {
    hash ^= char.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash;
}
