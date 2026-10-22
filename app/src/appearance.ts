// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { commands } from "./bindings";
import { layoutPref } from "./layoutPref";
import { drawPerson, type Density, type FontSlot, type Person } from "./theme/theme";
import { sayAboutThisMachine } from "./windowprefs";

/**
 * **The person's density, font override, motion, contrast and colours** (#2326, #2925, spec
 * #2313): settings every theme honours, kept on this machine in the layout file beside the text
 * sizes, under `appearance` (`{ "density": "compact", "fonts": { "terminal": "Iosevka Term" },
 * "motion": "reduced", "contrast": "more", "colours": { "accent.base": "#8f7bff" } }`).
 *
 * **Reduced motion and more contrast are layered on the system's**: either asking is enough.
 * The colours and the contrast are drawn by the engine into each theme it answers with, held to
 * the floors; so a change here asks for the theme in force again (`theme.ts`, `onPersonLook`).
 *
 * **The core's theme engine reads them** (`purlis_core::theme::Person`), at creation from the
 * file and here from what Settings set (`theme_person`), and answers the person's layer, which
 * `theme.ts` draws over the theme in force. This module only keeps what the person chose and
 * hands it to the engine: a family name is held to the engine's grammar there, not here.
 */
export type Appearance = {
  density?: Density;
  fonts?: Partial<Record<FontSlot, string>>;
  motion?: "system" | "reduced";
  contrast?: "standard" | "more";
  colours?: Record<string, string>;
};

/** The value as the file holds it: an object, or nothing. Its fields are the engine's to judge. */
function loadAppearance(raw: unknown): { value: Appearance; said: string[] } {
  const held =
    raw !== null && typeof raw === "object" && !Array.isArray(raw)
      ? (raw as { appearance?: unknown }).appearance
      : undefined;
  if (held === null || typeof held !== "object" || Array.isArray(held))
    return { value: {}, said: [] };
  return { value: held as Appearance, said: [] };
}

/** What the file keeps: nothing where it is the default. */
function written(value: Appearance): Appearance | undefined {
  const fonts = Object.fromEntries(
    Object.entries(value.fonts ?? {}).filter(([, family]) => (family ?? "").trim() !== ""),
  );
  const colours = Object.fromEntries(
    Object.entries(value.colours ?? {}).filter(([, colour]) => (colour ?? "").trim() !== ""),
  );
  const kept: Appearance = {
    ...(value.density === "compact" && { density: "compact" }),
    ...(Object.keys(fonts).length > 0 && { fonts }),
    ...(value.motion === "reduced" && { motion: "reduced" }),
    ...(value.contrast === "more" && { contrast: "more" }),
    ...(Object.keys(colours).length > 0 && { colours }),
  };
  return Object.keys(kept).length > 0 ? kept : undefined;
}

export const APPEARANCE = layoutPref<Appearance, "appearance">({
  key: "appearance",
  fallback: {},
  load: loadAppearance,
  same: (one, other) => JSON.stringify(written(one)) === JSON.stringify(written(other)),
  written,
  remedy: (where) =>
    `fix ${where}, or set the density, fonts, motion, contrast and colours in Settings, which rewrites it`,
  settings: "you.look",
});

/** What the person chose. */
export function useAppearance(): Appearance {
  return APPEARANCE.use();
}

/** Asks the engine for the person's layer of `value` and draws it; what it put right is said. */
async function drawAppearance(value: Appearance): Promise<void> {
  let answered: Awaited<ReturnType<typeof commands.themePerson>>;
  try {
    const kept = written(value);
    answered = await commands.themePerson(kept === undefined ? null : JSON.stringify(kept));
  } catch {
    return;
  }
  if (answered === null || typeof answered !== "object") return;
  const person = answered as Person & { complaints: string[] };
  drawPerson({
    density: person.density,
    fonts: person.fonts,
    motion: person.motion,
    contrast: person.contrast,
    colours: person.colours,
    complaints: person.complaints,
  });
  sayAppearance(person.complaints);
}

/** Says what the engine put right in the person's settings, or takes it back: at launch, from
 *  what the window was handed, and after each change. */
export function sayAppearance(complaints: readonly string[] | undefined): void {
  sayAboutThisMachine(
    "appearance",
    complaints && complaints.length > 0
      ? {
          severity: "warn",
          detail: complaints.join("; "),
          remedy: "change it in Settings",
          settings: settingsFor(complaints),
        }
      : undefined,
  );
}

/** **The Settings group a complaint about the person's settings links to** (#2925): Your
 *  colours for a colour, Motion and contrast for those two, else Density and fonts. */
export function settingsFor(complaints: readonly string[]): string {
  const first = complaints[0] ?? "";
  if (/^(your colour for|colours is|[a-z0-9.-]+ is not a colour token)/.test(first))
    return "you.colours";
  if (/^(motion|contrast) is/.test(first)) return "you.motion";
  return "you.look";
}

/** The person's density. */
export function setDensity(density: Density): void {
  const next = { ...APPEARANCE.value(), density };
  APPEARANCE.set(next);
  void drawAppearance(next);
}

/** The family the person names for `slot`; empty is the theme's. */
export function setFontOverride(slot: FontSlot, family: string): void {
  const now = APPEARANCE.value();
  const next = { ...now, fonts: { ...now.fonts, [slot]: family } };
  APPEARANCE.set(next);
  void drawAppearance(next);
}

/** Whether motion is off in purlis, whatever the system says (#2925). */
export function setReduceMotion(reduced: boolean): void {
  const next: Appearance = { ...APPEARANCE.value(), motion: reduced ? "reduced" : "system" };
  APPEARANCE.set(next);
  void drawAppearance(next);
}

/** The contrast purlis holds, at least (#2925): `more` raises the floors. */
export function setMinimumContrast(contrast: "standard" | "more"): void {
  const next: Appearance = { ...APPEARANCE.value(), contrast };
  APPEARANCE.set(next);
  void drawAppearance(next);
}

/** The person's colours, over every theme (#2925): each token's, replacing what was set. */
export function setColours(colours: Readonly<Record<string, string>>): void {
  const next: Appearance = { ...APPEARANCE.value(), colours: { ...colours } };
  APPEARANCE.set(next);
  void drawAppearance(next);
}
