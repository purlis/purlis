// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { layoutPref } from "./layoutPref";

/**
 * **The extensions whose offers the person stopped** (#4769, prototype `view-open-beside` Q3 A):
 * an offer note has Stop offers from <extension>, which holds every offer from it until
 * Settings › Extensions turns its offers back on. It is the person's preference and no new
 * capability (ADR 0041): it only holds notes back. Kept in the layout file under `offers`
 * (`layout.json`, `regions.ts`), beside the window's other choices on this machine; an empty
 * list is the default, which the file leaves out.
 */

/** The ids as a layout document holds them, and what had to be put right. */
export function loadOffersOff(raw: unknown): { value: readonly string[]; said: string[] } {
  const held =
    raw !== null && typeof raw === "object" && !Array.isArray(raw)
      ? (raw as { offers?: unknown }).offers
      : undefined;
  if (held === undefined) return { value: [], said: [] };
  const off =
    held !== null && typeof held === "object" && !Array.isArray(held)
      ? (held as { off?: unknown }).off
      : undefined;
  if (!Array.isArray(off))
    return {
      value: [],
      said: [`"offers" is not { "off": [...] }, so every extension may offer its views`],
    };
  const ids = off.filter((one): one is string => typeof one === "string");
  return ids.length === off.length
    ? { value: ids, said: [] }
    : {
        value: ids,
        said: [`"offers.off" holds something that is not an extension's id, which was left out`],
      };
}

/** The store (`layoutPref.ts`), under `offers`. */
export const OFFERS = layoutPref<readonly string[], "offers">({
  key: "offers",
  fallback: [],
  load: loadOffersOff,
  same: (one, other) => one.length === other.length && one.every((id, at) => other[at] === id),
  written: (off) => (off.length === 0 ? undefined : { off: [...off] }),
  remedy: (where) =>
    `fix ${where}, or turn an extension's offers on or off in Settings › Extensions, which rewrites it`,
});

/** The extensions whose offers are stopped now, for a component that redraws when it changes. */
export const useOffersOff = (): readonly string[] => OFFERS.use();

/** Stops or turns back on `extension`'s offers, and writes the choice to the layout file. */
export function offersFrom(extension: string, on: boolean): void {
  const now = OFFERS.value();
  const next = on ? now.filter((id) => id !== extension) : [...new Set([...now, extension])];
  OFFERS.set(next);
}
