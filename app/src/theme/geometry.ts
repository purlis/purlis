// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * The core geometry: the names of the sizes every theme draws from (ADR 0094, ADR 0097).
 *
 * **Why this module exists.** Colour and motion were tokens with guards, and size was not: 132
 * paddings, 20 font sizes and 11 radii, each a local decision, so every view drew its own
 * controls and they drifted apart. These tokens are the one scale a view may draw from.
 *
 * **A theme sets them as bounded numbers, and the core writes them** (#2326). A theme gives a
 * geometry token a number within the range the core's theme engine documents
 * (`purlis_core::theme::geometry`), and its fonts as woff2 files in its folder; the engine writes
 * each property here from those and hands the window the result for both densities, comfortable
 * and compact (`theme.ts` puts the person's on the document). A value outside its bound refuses
 * the theme. The bounds hold each token on its own: a theme stays within the range every view
 * was drawn to take, though tokens are not checked against one another.
 *
 * This list is the contract: `geometry.css` declares exactly these names, holding Purlis
 * Classic's values, no other stylesheet declares one, and `docs/design-system.md` lists every
 * one. The engine names the same list (`purlis_core::theme::vocabulary::GEOMETRY`, held to this
 * one by `core.test.ts`), and its own test holds what it writes for Purlis Classic equal to
 * `geometry.css`.
 */

/** Each group of core tokens, and the custom properties in it. */
export const GEOMETRY = {
  /** The one spacing scale: 0, 2, 4, 6, 8, 12, 16, 24 and 32px at the default text size. */
  space: [
    "--space-0",
    "--space-1",
    "--space-2",
    "--space-3",
    "--space-4",
    "--space-5",
    "--space-6",
    "--space-7",
    "--space-8",
  ],
  /** The 7-step type scale: 10 (a tight secondary label only), 11, 12, 13, 14 (the window's
   *  text), 16 and 20px. */
  type: [
    "--font-size-2xs",
    "--font-size-xs",
    "--font-size-sm",
    "--font-size-md",
    "--font-size-base",
    "--font-size-lg",
    "--font-size-xl",
  ],
  /** The three weights of the window's words: regular, medium (a form's label, #2317) and the
   *  one strong (#1920). */
  weight: ["--font-weight-regular", "--font-weight-medium", "--font-weight-strong"],
  /** Line heights, as a share of the text's size: a compact line, and the window's own. */
  leading: ["--line-height-tight", "--line-height-body"],
  /** The window's own family, and the one monospace family: a command, a path, a host
   *  (`ui-text-code`). The terminal's family is xterm's, not a property. */
  family: ["--font-ui", "--font-mono"],
  /** Corners: 3px (checks, chips), 4px (controls), 6px (every overlay), and 8px. */
  radius: [
    "--radius-sm",
    "--radius-md",
    "--radius-control",
    "--radius-lg",
    "--radius-xl",
    "--radius-full",
  ],
  /** One height for every control (28px) and one for every strip and pane header (21px). */
  size: ["--size-control", "--size-chrome"],
  /** The window's geometry (#2805, #2913): the gap between its regions, a region panel's
   *  corners, and the hairline between regions. Classic is 0, 0 and 1: flush, as it always
   *  drew. */
  window: ["--window-gutter", "--panel-corner-radius", "--panel-border-width"],
  /** The separator part's width (#2912): the line between two groups inside a surface, 0 for
   *  none; its colour is the separator family's. */
  separator: ["--separator-width"],
  /** The two focus-ring rules: outside a control, inside a row or a tab. */
  focus: ["--focus-ring-width", "--focus-ring-offset", "--focus-ring-inset"],
  /** The line around a pane (#2914): 1px in Classic, none in Purlis. */
  pane: ["--pane-border-width"],
  /** How far a surface is lifted: a bar floating over a pane, and every overlay (a dialog, a
   *  popover, a menu, a tooltip), which is one surface at one lift (#1845). */
  elevation: ["--elevation-raised", "--elevation-overlay"],
} as const;

/** Every core geometry custom property. */
export const GEOMETRY_PROPERTIES: readonly string[] = Object.values(GEOMETRY).flat();

/** Whether a custom property is core geometry: written only from what the engine hands. */
export function isGeometry(name: string): boolean {
  return GEOMETRY_PROPERTIES.includes(name);
}
