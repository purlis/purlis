// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { commands } from "./bindings";
import { layoutPref } from "./layoutPref";
import { drawPerson, type Density, type FontSlot, type Person } from "./theme/theme";
import { sayAboutThisMachine } from "./windowprefs";

/**
 * **The person's density and font override** (#2326, spec #2313): settings every theme honours,
 * kept on this machine in the layout file beside the text sizes, under `appearance`
 * (`{ "density": "compact", "fonts": { "terminal": "Iosevka Term" } }`).
 *
 * **The core's theme engine reads them** (`purlis_core::theme::Person`), at creation from the
 * file and here from what Settings set (`theme_person`), and answers the person's layer, which
 * `theme.ts` draws over the theme in force. This module only keeps what the person chose and
 * hands it to the engine: a family name is held to the engine's grammar there, not here.
 */
export type Appearance = { density?: Density; fonts?: Partial<Record<FontSlot, string>> };

/** The value as the file holds it: an object, or nothing. Its fields are the engine's to judge. */
function loadAppearance(raw: unknown): { value: Appearance; said: string[] } {
  const held =
    raw !== null && typeof raw === "object" && !Array.isArray(raw)
      ? (raw as { appearance?: unknown }).appearance
      : undefined;
  if (held === null || typeof held !== "object" || Array.isArray(held)) return { value: {}, said: [] };
  return { value: held as Appearance, said: [] };
}

/** What the file keeps: nothing where it is the default. */
function written(value: Appearance): Appearance | undefined {
  const fonts = Object.fromEntries(
    Object.entries(value.fonts ?? {}).filter(([, family]) => (family ?? "").trim() !== ""),
  );
  const kept: Appearance = {
    ...(value.density === "compact" && { density: "compact" }),
    ...(Object.keys(fonts).length > 0 && { fonts }),
  };
  return Object.keys(kept).length > 0 ? kept : undefined;
}

export const APPEARANCE = layoutPref<Appearance, "appearance">({
  key: "appearance",
  fallback: {},
  load: loadAppearance,
  same: (one, other) => JSON.stringify(written(one)) === JSON.stringify(written(other)),
  written,
  remedy: (where) => `fix ${where}, or set the density and fonts in Settings, which rewrites it`,
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
    answered = await commands.themePerson(written(value) ?? null);
  } catch {
    return;
  }
  if (answered === null || typeof answered !== "object") return;
  const person = answered as Person & { complaints: string[] };
  drawPerson({ density: person.density, fonts: person.fonts, complaints: person.complaints });
  sayAboutThisMachine(
    "appearance",
    person.complaints.length > 0
      ? {
          severity: "warn",
          detail: person.complaints.join("; "),
          remedy: "change it in Settings",
          settings: "you.look",
        }
      : undefined,
  );
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
