// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  BUILT_IN,
  CLASSIC,
  CLASSIC_THEME,
  DEFAULT_THEME,
  SYSTEM,
  THEME_ALIASES,
  type Theme,
} from "./theme";

/**
 * **What the core's `theme_in_force` answers, for a test's window that picks only built-ins.**
 *
 * The real answer is the core's theme engine's (`purlis_core::theme::resolve`, whose own tests
 * hold every rule of it). A test of the window's other surfaces (a project switch, a workspace's
 * colour, the chrome in a theme) only needs the window handed what a built-in pick draws: the
 * built-in it names (by its id or an id it had before), the one of the system's appearance for
 * `system`, and the default for nothing picked, as a machine with no theme file and no extension
 * themes resolves.
 */
export function themeInForce(args: unknown): {
  theme: Theme;
  complaints: string[];
  follows_system: boolean;
} {
  const asked = (args ?? {}) as { pick?: string | null; light?: boolean };
  const pick = asked.pick ?? null;
  if (pick === SYSTEM)
    return {
      theme: BUILT_IN[asked.light === true ? "purlis-light" : "purlis-dark"],
      complaints: [],
      follows_system: true,
    };
  if (pick === CLASSIC) return { theme: CLASSIC_THEME, complaints: [], follows_system: false };
  const has = (table: object, key: string) => Object.prototype.hasOwnProperty.call(table, key);
  const id = pick !== null && has(THEME_ALIASES, pick) ? THEME_ALIASES[pick] : pick;
  const theme = id !== null && has(BUILT_IN, id) ? BUILT_IN[id] : DEFAULT_THEME;
  return { theme, complaints: [], follows_system: false };
}
