// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { layoutPref } from "./layoutPref";
import { sayAboutThisMachine } from "./windowprefs";

/**
 * **The status items the person hid** (N28, #2913): the status bar's contribution model lets the
 * person hide any item, purlis's own and an extension's alike, from the bar's own menu
 * (`ui/status/StatusBar.tsx`), and bring it back from the same menu.
 *
 * **Kept in the layout file, under `status`** (`layout.json`, `regions.ts`): how one person likes
 * their window, on this machine, as the Chats list's drawing is. An item is named by its id
 * (`purlis.notices`, `extension:<id>/<badge>`), so an item that is not there today (an extension
 * turned off) stays hidden when it comes back. Bounded: at most {@link MOST_HIDDEN} ids of at most
 * {@link MOST_ID} characters, and a value that is not one is the default, said in the Inbox.
 */
export type StatusBarPrefs = { hidden: readonly string[] };

export const DEFAULT_STATUS_BAR: StatusBarPrefs = { hidden: [] };

/** The most items the file keeps hidden. */
export const MOST_HIDDEN = 64;

/** The longest id the file keeps. */
export const MOST_ID = 200;

/** The preferences as a layout document holds them, and what had to be put right. */
export function loadStatusBar(raw: unknown): { prefs: StatusBarPrefs; said: string[] } {
  const said: string[] = [];
  const held =
    raw !== null && typeof raw === "object" && !Array.isArray(raw)
      ? (raw as { status?: unknown }).status
      : undefined;
  if (held === undefined) return { prefs: DEFAULT_STATUS_BAR, said };
  const hidden =
    held !== null && typeof held === "object" && !Array.isArray(held)
      ? (held as { hidden?: unknown }).hidden
      : undefined;
  if (!Array.isArray(hidden)) {
    said.push(
      `"status" ${JSON.stringify(held)} is not the status items you hid, so every item is shown`,
    );
    return { prefs: DEFAULT_STATUS_BAR, said };
  }
  const kept: string[] = [];
  for (const one of hidden) {
    if (typeof one !== "string" || one === "" || one.length > MOST_ID) {
      said.push(`"status.hidden" ${JSON.stringify(one)} is not a status item's id, so it is shown`);
      continue;
    }
    if (kept.includes(one)) continue;
    if (kept.length === MOST_HIDDEN) {
      said.push(`"status.hidden" keeps at most ${MOST_HIDDEN} items, so the rest are shown`);
      break;
    }
    kept.push(one);
  }
  return { prefs: { hidden: kept }, said };
}

/** The store (`layoutPref.ts`), under `status`. */
export const STATUS_BAR = layoutPref<StatusBarPrefs, "status">({
  key: "status",
  fallback: DEFAULT_STATUS_BAR,
  load: (raw) => {
    const { prefs, said } = loadStatusBar(raw);
    return { value: prefs, said };
  },
  same: (one, other) =>
    one.hidden.length === other.hidden.length &&
    one.hidden.every((id, at) => other.hidden[at] === id),
  written: (prefs) => (prefs.hidden.length === 0 ? undefined : { hidden: [...prefs.hidden] }),
  remedy: (where) => `fix ${where}, or show the item again from the status bar's menu`,
});

/** The ids of the status items hidden now, for a component that redraws when they change. */
export function useHiddenStatusItems(): readonly string[] {
  return STATUS_BAR.use().hidden;
}

/** The Inbox's subject for an item the bound kept from being hidden. */
export const HIDING_REFUSED = "status-hidden";

/**
 * Hides the status item `id`, or shows it again; tells every listener when it changed. One past
 * the bound stays shown, and the Inbox says why rather than the check doing nothing.
 */
export function hideStatusItem(id: string, hidden: boolean): void {
  const now = STATUS_BAR.value().hidden.filter((one) => one !== id);
  const fits = now.length < MOST_HIDDEN && id.length <= MOST_ID;
  if (hidden && fits) now.push(id);
  STATUS_BAR.set({ hidden: now });
  sayAboutThisMachine(
    HIDING_REFUSED,
    hidden && !fits
      ? {
          severity: "warn",
          detail: `purlis keeps at most ${MOST_HIDDEN} status items hidden, so this one stays shown`,
          remedy:
            "show an item you no longer need hidden from the status bar's menu, then hide this one",
        }
      : undefined,
  );
}
