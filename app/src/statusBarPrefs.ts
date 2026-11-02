// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { layoutPref } from "./layoutPref";
import { sayAboutThisMachine } from "./windowprefs";

/**
 * **The status items the person hid** (N28, #2913): the status bar's contribution model lets the
 * person hide any item, purlis's own and an extension's alike, from the bar's own menu
 * (`ui/status/StatusBar.tsx`), and bring it back from the same menu.
 *
 * **And the order they gave them** (#3030), a list of ids, read against what the bar draws today:
 * an id it names takes that place, an item it never named stays beside the one it follows by
 * priority (`placed`). A move changes the one item's place in the list, so an item absent today
 * keeps its own, beside the ones it stood beside, for when it comes back. An item keeps its end:
 * the two ends share the list and never meet in it.
 *
 * **Kept in the layout file, under `status`** (`layout.json`, `regions.ts`): how one person likes
 * their window, on this machine, as the Chats list's drawing is. An item is named by its id
 * (`purlis.notices`, `extension:<id>/<badge>`), so an item that is not there today (an extension
 * turned off) stays hidden when it comes back. Bounded: at most {@link MOST_HIDDEN} ids hidden and
 * {@link MOST_ORDERED} in order, of at most {@link MOST_ID} characters each, and a value that is
 * not one is the default, said in the Inbox.
 */
export type StatusBarPrefs = { hidden: readonly string[]; order: readonly string[] };

export const DEFAULT_STATUS_BAR: StatusBarPrefs = { hidden: [], order: [] };

/** The most items the file keeps hidden. */
export const MOST_HIDDEN = 64;

/** The most items the file keeps an order for. */
export const MOST_ORDERED = 64;

/** The longest id the file keeps. */
export const MOST_ID = 200;

/** The ids of a list the file holds, each once and within the bounds, and what was left out. */
function idsIn(
  list: readonly unknown[],
  field: string,
  most: number,
  then: { one: string; rest: string },
  said: string[],
): string[] {
  const kept: string[] = [];
  for (const one of list) {
    if (typeof one !== "string" || one === "" || one.length > MOST_ID) {
      said.push(`"${field}" ${JSON.stringify(one)} is not a status item's id, so ${then.one}`);
      continue;
    }
    if (kept.includes(one)) continue;
    if (kept.length === most) {
      said.push(`"${field}" keeps at most ${most} items, so ${then.rest}`);
      break;
    }
    kept.push(one);
  }
  return kept;
}

/** The preferences as a layout document holds them, and what had to be put right. */
export function loadStatusBar(raw: unknown): { prefs: StatusBarPrefs; said: string[] } {
  const said: string[] = [];
  const held =
    raw !== null && typeof raw === "object" && !Array.isArray(raw)
      ? (raw as { status?: unknown }).status
      : undefined;
  if (held === undefined) return { prefs: DEFAULT_STATUS_BAR, said };
  if (held === null || typeof held !== "object" || Array.isArray(held)) {
    said.push(
      `"status" ${JSON.stringify(held)} is not the status items you hid, so every item is shown`,
    );
    return { prefs: DEFAULT_STATUS_BAR, said };
  }
  const { hidden = [], order = [] } = held as { hidden?: unknown; order?: unknown };
  if (!Array.isArray(hidden))
    said.push(
      `"status.hidden" ${JSON.stringify(hidden)} is not the status items you hid, so every item is shown`,
    );
  if (!Array.isArray(order))
    said.push(
      `"status.order" ${JSON.stringify(order)} is not an order of status items, so the bar keeps its own`,
    );
  return {
    prefs: {
      hidden: Array.isArray(hidden)
        ? idsIn(
            hidden,
            "status.hidden",
            MOST_HIDDEN,
            { one: "it is shown", rest: "the rest are shown" },
            said,
          )
        : [],
      order: Array.isArray(order)
        ? idsIn(
            order,
            "status.order",
            MOST_ORDERED,
            { one: "it keeps its own place", rest: "the rest keep their own place" },
            said,
          )
        : [],
    },
    said,
  };
}

const sameIds = (one: readonly string[], other: readonly string[]) =>
  one.length === other.length && one.every((id, at) => other[at] === id);

/** The store (`layoutPref.ts`), under `status`. */
export const STATUS_BAR = layoutPref<StatusBarPrefs, "status">({
  key: "status",
  fallback: DEFAULT_STATUS_BAR,
  load: (raw) => {
    const { prefs, said } = loadStatusBar(raw);
    return { value: prefs, said };
  },
  same: (one, other) => sameIds(one.hidden, other.hidden) && sameIds(one.order, other.order),
  written: (prefs) =>
    prefs.hidden.length === 0 && prefs.order.length === 0
      ? undefined
      : {
          hidden: [...prefs.hidden],
          ...(prefs.order.length === 0 ? {} : { order: [...prefs.order] }),
        },
  remedy: (where) => `fix ${where}, or show the item again from the status bar's menu`,
});

/** The ids of the status items hidden now, for a component that redraws when they change. */
export function useHiddenStatusItems(): readonly string[] {
  return STATUS_BAR.use().hidden;
}

/** The order the person gave the status items, for a component that redraws when it changes. */
export function useStatusOrder(): readonly string[] {
  return STATUS_BAR.use().order;
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
  STATUS_BAR.set({ ...STATUS_BAR.value(), hidden: now });
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

/**
 * **The order after `moved` is put where `onto` stands** on an end whose items are drawn as
 * `shown`, left to right. What the end draws and the order never named joins it where it is drawn;
 * then only `moved` changes place, so every other item, one absent today among them, keeps the
 * neighbours it had: going left it lands just before `onto`, going right just after.
 */
export function reordered(
  order: readonly string[],
  shown: readonly string[],
  moved: string,
  onto: string,
): string[] {
  const listed = [...order];
  shown.forEach((id, at) => {
    if (!listed.includes(id))
      listed.splice(at === 0 ? 0 : listed.indexOf(shown[at - 1]) + 1, 0, id);
  });
  const rest = listed.filter((id) => id !== moved);
  const rightwards = shown.indexOf(moved) < shown.indexOf(onto);
  rest.splice(rest.indexOf(onto) + (rightwards ? 1 : 0), 0, moved);
  return rest;
}

/**
 * Puts the status item `id` where `onto` stands on its end, whose items are drawn as `shown`, and
 * tells every listener. Past the bound the ids the end does not draw go, last first: what is lost
 * is the place of an item that is not there, and it comes back beside its neighbour by priority.
 * An id too long for the file stays out of it, so the file is never one the next launch refuses.
 */
export function moveStatusItem(id: string, onto: string, shown: readonly string[]): void {
  const prefs = STATUS_BAR.value();
  const order = reordered(prefs.order, shown, id, onto).filter((one) => one.length <= MOST_ID);
  for (let at = order.length - 1; at >= 0 && order.length > MOST_ORDERED; at -= 1)
    if (!shown.includes(order[at])) order.splice(at, 1);
  STATUS_BAR.set({ ...prefs, order: order.slice(0, MOST_ORDERED) });
}
