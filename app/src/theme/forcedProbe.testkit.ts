// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * Test-only: **what the window, and each family, draws for `forced-colors.e2e.ts` to read**
 * (#3481, operator ruling 2026-10-26: two family pull requests never touch the same lines).
 *
 * The window's own probe is `theme/forced-colors.probe.ts`, beside `theme/forced-colors.css`; a
 * family's is `ui/<family>/forced-colors.probe.ts`, beside its `forced-colors.css` and its
 * `theme.testkit.ts`'s `forced`. Each default-exports a {@link ForcedProbe}. The spec finds them
 * on disk when it loads (`e2e/forcedProbes.ts`), since it runs under WebdriverIO in the built
 * WebView and `import.meta.glob` is Vite's: a family adds its file, and no list.
 *
 * A probe imports nothing at run time: the spec loads it as it is.
 */

/** The system colours the spec resolves in the WebView, each by its CSS keyword. */
export const SYSTEM_COLOURS = [
  "Canvas",
  "CanvasText",
  "Highlight",
  "HighlightText",
  "SelectedItem",
  "SelectedItemText",
  "Mark",
  "MarkText",
] as const;

export type SystemColour = (typeof SYSTEM_COLOURS)[number];

/** What the spec reads of each probed control's computed style. */
export type ForcedRead =
  | "color"
  | "background"
  | "border"
  | "outlineStyle"
  | "outlineWidth"
  | "outlineColor"
  | "outlineOffset"
  | "decoration"
  | "shadow"
  | "focusVisible";

/** A value read: as the engine computes it (`"2px"`, `"dashed"`), or the system colour it must
 *  resolve to. */
export type ForcedValue = string | { readonly system: SystemColour };

/** One control's values, by what is read. */
export type ForcedValues = Readonly<Partial<Record<ForcedRead, ForcedValue>>>;

export interface ForcedProbe {
  /** The controls, with the window's own attributes, as their components draw them. Each one
   *  read carries a `data-p` naming it, unique within this probe; the spec adds the `ds-` and
   *  `dp-` classes a `data-scope` and `data-part` key the built stylesheet on (#4351). */
  readonly markup: string;
  /** With forced colours active: each control's values, by its `data-p`. */
  readonly forced: Readonly<Record<string, ForcedValues>>;
  /** Without them, what each of these controls is not, so it is the query that draws it. */
  readonly unforced?: Readonly<Record<string, ForcedValues>>;
  /** The `data-p` of the control to give the keyboard's focus before reading. One probe in all
   *  may name one, as the window has one focus. */
  readonly focus?: string;
}
