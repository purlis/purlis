// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * The core geometry: the names of the sizes every theme shares (ADR 0094).
 *
 * **Why this module exists.** Colour and motion were tokens with guards, and size was not: 132
 * paddings, 20 font sizes and 11 radii, each a local decision, so every view drew its own
 * controls and they drifted apart. These tokens are the one scale a view may draw from.
 *
 * **Themes own colour, shadow colour and motion; core owns geometry.** A theme is data an
 * author or an extension writes (`theme.ts`), and a theme that could move a control's height
 * could break the alignment of every view that sits on it. So geometry is not in a theme's
 * vocabulary at all. The values are written once, in `geometry.css`, in the stylesheet's
 * `theme` layer, and nothing a theme file says is ever written to one of these properties:
 * `geometry.test.ts` holds that, for the built-ins and for a theme that tries.
 *
 * **Density is a later user setting, not a theme property.** When it comes it swaps these
 * values for another set; a theme still never sees them.
 *
 * This list is the contract: `geometry.css` declares exactly these names, no other stylesheet
 * declares one, and `docs/design-system.md` lists every one. Values live in the CSS only, so
 * there is no second copy to drift. The core's theme engine refuses each name in a theme file
 * (`purlis_core::theme::vocabulary::GEOMETRY`, held to this list by `core.test.ts`).
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
  /** The one monospace family: a command, a path, a host (`ui-text-code`). */
  family: ["--font-mono"],
  /** Corners: 3px (checks, chips), 4px (controls), 6px (every overlay), and 8px. */
  radius: ["--radius-sm", "--radius-md", "--radius-lg", "--radius-xl", "--radius-full"],
  /** One height for every control (28px) and one for every strip and pane header (21px). */
  size: ["--size-control", "--size-chrome"],
  /** The two focus-ring rules: outside a control, inside a row or a tab. */
  focus: ["--focus-ring-width", "--focus-ring-offset", "--focus-ring-inset"],
  /** How far a surface is lifted: a bar floating over a pane, and every overlay (a dialog, a
   *  popover, a menu, a tooltip), which is one surface at one lift (#1845). */
  elevation: ["--elevation-raised", "--elevation-overlay"],
} as const;

/** Every core geometry custom property. */
export const GEOMETRY_PROPERTIES: readonly string[] = Object.values(GEOMETRY).flat();

/** Whether a custom property is core geometry, which no theme may set. */
export function isGeometry(name: string): boolean {
  return GEOMETRY_PROPERTIES.includes(name);
}
