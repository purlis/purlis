// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { type Remind, REMINDS } from "./handBounce";
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
  /**
   * **The Should-know notifications the person turned off** (#6019, ADR 0121 decision 22), by
   * the kind of update each tells of: a failed task (`task-failed`) and a long finish while away
   * (`task-done`). None until they turn one off. A Blocking ask is no source: it always
   * notifies. The core reads it here (`asknotify.rs`, `Sources::of_layout`).
   */
  off: readonly Source[];
  /**
   * **A sound for an Expiring ask about to fail** (ADR 0121 decision 12): off until the person
   * turns it on. Kept for the sound, which purlis does not play yet (#6110).
   */
  expiringSound: boolean;
  /**
   * **How often the ✋ reminds of a Blocking ask that waits** (#6344, ADR 0121 amended
   * 2026-11-12, "Remind me"): `"gently"`, the backoff, until the person changes it; `"often"`
   * every 5 minutes; `"off"` never, though a new ask still bounces. The window alone reads it
   * (`handBounce.ts`).
   */
  remind: Remind;
};

/** The kinds of update whose notification can be turned off: every Should-know one that notifies. */
export const SOURCES = ["task-failed", "task-done"] as const;
export type Source = (typeof SOURCES)[number];

const isSource = (kind: unknown): kind is Source => SOURCES.includes(kind as Source);

export const DEFAULT_NOTIFICATIONS: NotificationPrefs = {
  quiet: { on: false, from: "22:00", to: "07:00" },
  finishedAfter: 5,
  off: [],
  expiringSound: false,
  remind: "gently",
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
  let off: Source[] = [];
  if (Array.isArray(held.off)) {
    off = SOURCES.filter((kind) => (held.off as unknown[]).includes(kind));
    const strays = held.off.filter((kind) => !isSource(kind));
    if (strays.length > 0)
      said.push(
        `"notifications.off" names ${JSON.stringify(strays)}, which no notification is, so they are left out`,
      );
  } else if (held.off !== undefined)
    said.push(
      `"notifications.off" ${JSON.stringify(held.off)} is not a list of kinds, so every notification is on`,
    );
  let expiringSound = DEFAULT_NOTIFICATIONS.expiringSound;
  if (typeof held.expiringSound === "boolean") expiringSound = held.expiringSound;
  else if (held.expiringSound !== undefined)
    said.push(
      `"notifications.expiringSound" ${JSON.stringify(held.expiringSound)} is not true or false, so it is false`,
    );
  let remind = DEFAULT_NOTIFICATIONS.remind;
  if (REMINDS.includes(held.remind as Remind)) remind = held.remind as Remind;
  else if (held.remind !== undefined)
    said.push(
      `"notifications.remind" ${JSON.stringify(held.remind)} is not "off", "gently" or "often", so it is "gently"`,
    );
  return {
    prefs: {
      quiet: { on, from: time("from"), to: time("to") },
      finishedAfter,
      off,
      expiringSound,
      remind,
    },
    said,
  };
}

const sameQuiet = (one: QuietHours, other: QuietHours) =>
  one.on === other.on && one.from === other.from && one.to === other.to;

const same = (one: NotificationPrefs, other: NotificationPrefs) =>
  sameQuiet(one.quiet, other.quiet) &&
  one.finishedAfter === other.finishedAfter &&
  one.off.join() === other.off.join() &&
  one.expiringSound === other.expiringSound &&
  one.remind === other.remind;

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
    if (prefs.off.length > 0) kept.off = prefs.off;
    if (prefs.expiringSound) kept.expiringSound = true;
    if (prefs.remind !== DEFAULT_NOTIFICATIONS.remind) kept.remind = prefs.remind;
    return Object.keys(kept).length === 0 ? undefined : kept;
  },
  remedy: (where) => `fix ${where}, or change Attention in Settings, which rewrites it`,
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

/** Turns the notification of `source` on or off, keeping the kinds in their one order. */
export function setSourceOn(source: Source, on: boolean): void {
  const was = notificationPrefs().off;
  const off = SOURCES.filter((kind) => (kind === source ? !on : was.includes(kind)));
  NOTIFICATIONS.set({ ...notificationPrefs(), off });
}

/** Turns the Expiring sound on or off. */
export function setExpiringSound(on: boolean): void {
  NOTIFICATIONS.set({ ...notificationPrefs(), expiringSound: on });
}

/** Changes how often the ✋ reminds of a Blocking ask that waits. */
export function setRemind(remind: Remind): void {
  NOTIFICATIONS.set({ ...notificationPrefs(), remind });
}
