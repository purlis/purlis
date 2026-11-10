// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { layoutPref } from "./layoutPref";

/**
 * **When the asks' system notifications keep quiet** (#1989, Q66): quiet hours, from a time of
 * day until another, on this machine's clock. In them no notification is shown and an ask
 * waits in the Inbox; when they end, one notification names the chats still waiting. Off
 * until the person turns them on.
 *
 * **Kept in the layout file, beside the text sizes and your editor** (`layout.json`,
 * `regions.ts`), under `notifications`: how one person likes to be interrupted, on this
 * machine. The core reads it there each time it would send one (`asknotify.rs`,
 * `QuietHours::of_layout`), so what is written here is what it keeps to. Each field falls back
 * on its own, and a value that is not one is the default's, said in the Inbox.
 */
export type QuietHours = {
  on: boolean;
  /** When they start, `HH:MM` on a 24-hour clock. */
  from: string;
  /** When they end: before `from`, they run across midnight. */
  to: string;
};

export type NotificationPrefs = {
  quiet: QuietHours;
  /**
   * **How long a task works, in whole minutes, before its finish notifies** while the person is
   * away from the window (#6005, ADR 0121 decision 3): five until they change it. A failure
   * always notifies. The core reads it here (`asknotify.rs`, `finished_after_of_layout`).
   */
  finishedAfter: number;
};

export const DEFAULT_NOTIFICATIONS: NotificationPrefs = {
  quiet: { on: false, from: "22:00", to: "07:00" },
  finishedAfter: 5,
};

/** Whether `value` is a long finish as the core reads one: a whole number of minutes above none. */
export const isMinutes = (value: unknown): value is number =>
  typeof value === "number" && Number.isInteger(value) && value > 0;

/** Whether `text` is a time of day as the core reads one: `HH:MM`, 00:00 to 23:59. */
export const isTimeOfDay = (text: unknown): text is string =>
  typeof text === "string" && /^([01]\d|2[0-3]):[0-5]\d$/.test(text);

const isObject = (raw: unknown): raw is Record<string, unknown> =>
  raw !== null && typeof raw === "object" && !Array.isArray(raw);

/** The preferences as a layout document holds them, and what had to be put right. */
export function loadNotifications(raw: unknown): { prefs: NotificationPrefs; said: string[] } {
  const said: string[] = [];
  const held = isObject(raw) ? raw.notifications : undefined;
  if (held === undefined) return { prefs: DEFAULT_NOTIFICATIONS, said };
  const quiet = isObject(held) ? (held.quiet ?? {}) : held;
  if (!isObject(held) || !isObject(quiet)) {
    said.push(
      `"notifications" ${JSON.stringify(held)} is not when notifications keep quiet, so they never do`,
    );
    return { prefs: DEFAULT_NOTIFICATIONS, said };
  }
  const fallback = DEFAULT_NOTIFICATIONS.quiet;
  let on = fallback.on;
  if (typeof quiet.on === "boolean") on = quiet.on;
  else if (quiet.on !== undefined)
    said.push(
      `"notifications.quiet.on" ${JSON.stringify(quiet.on)} is not true or false, so it is false`,
    );
  const time = (key: "from" | "to") => {
    const value = quiet[key];
    if (isTimeOfDay(value)) return value;
    if (value !== undefined)
      said.push(
        `"notifications.quiet.${key}" ${JSON.stringify(value)} is not a time like 07:00, so it is ${fallback[key]}`,
      );
    return fallback[key];
  };
  let finishedAfter = DEFAULT_NOTIFICATIONS.finishedAfter;
  if (isMinutes(held.finishedAfter)) finishedAfter = held.finishedAfter;
  else if (held.finishedAfter !== undefined)
    said.push(
      `"notifications.finishedAfter" ${JSON.stringify(held.finishedAfter)} is not a whole number of minutes, so it is ${finishedAfter}`,
    );
  return { prefs: { quiet: { on, from: time("from"), to: time("to") }, finishedAfter }, said };
}

const sameQuiet = (one: QuietHours, other: QuietHours) =>
  one.on === other.on && one.from === other.from && one.to === other.to;

const same = (one: NotificationPrefs, other: NotificationPrefs) =>
  sameQuiet(one.quiet, other.quiet) && one.finishedAfter === other.finishedAfter;

/** The store (`layoutPref.ts`), under `notifications`. */
export const NOTIFICATIONS = layoutPref<NotificationPrefs, "notifications">({
  key: "notifications",
  fallback: DEFAULT_NOTIFICATIONS,
  load: (raw) => {
    const { prefs, said } = loadNotifications(raw);
    return { value: prefs, said };
  },
  same,
  // Each field only where it is not the default, and nothing at all where none is.
  written: (prefs) => {
    const kept: Partial<NotificationPrefs> = {};
    if (!sameQuiet(prefs.quiet, DEFAULT_NOTIFICATIONS.quiet)) kept.quiet = prefs.quiet;
    if (prefs.finishedAfter !== DEFAULT_NOTIFICATIONS.finishedAfter)
      kept.finishedAfter = prefs.finishedAfter;
    return Object.keys(kept).length === 0 ? undefined : kept;
  },
  remedy: (where) => `fix ${where}, or change Notifications in Settings, which rewrites it`,
  settings: "you.notifications",
});

/** When notifications keep quiet now. */
export function notificationPrefs(): NotificationPrefs {
  return NOTIFICATIONS.value();
}

/** Changes the quiet hours; tells every listener, which writes the file, when they changed. */
export function setQuietHours(to: Partial<QuietHours>): void {
  NOTIFICATIONS.set({ ...notificationPrefs(), quiet: { ...notificationPrefs().quiet, ...to } });
}

/** Changes how long a task works before its finish notifies, in whole minutes. */
export function setFinishedAfter(minutes: number): void {
  NOTIFICATIONS.set({ ...notificationPrefs(), finishedAfter: minutes });
}

/** {@link notificationPrefs}, for a component that redraws when they change. */
export function useNotificationPrefs(): NotificationPrefs {
  return NOTIFICATIONS.use();
}
