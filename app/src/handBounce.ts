// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The ✋'s reminder bounce** (#6343, ADR 0121 amended 2026-11-12, "The ✋'s reminder bounce"):
 * the one movement in the window that calls the person, as a schedule. Pure and clock-injected:
 * every event carries its own time, so a test drives it with a fake clock and nothing here reads
 * one. `useHandBounce` (`useHandBounce.ts`) feeds it from the timer and the handlers.
 *
 * - **On a new ask** the ✋ bounces (3 times, the stylesheet's `purlis-hand-bounce`).
 * - **As a reminder**, while a Blocking ask waits in the project in front, it bounces again at
 *   the pace the person picked in Settings › Attention › Remind me (#6344, {@link Remind}):
 *   Gently, the default, on a backoff of 5, 10, 20, 40 minutes, then every 60
 *   ({@link REMIND_AFTER_MINUTES}); Often every 5 minutes; Off never, though a new ask still
 *   bounces.
 * - **Resets** to 5 minutes on a new ask, any answer, or Asks opening.
 * - **Stops** when nothing waits.
 * - **Holds** ({@link Holds}): a reminder due while held is not played and not made up later; the
 *   schedule goes on from the time it was due.
 *
 * An Expiring ask's sound is its own (decision 12) and is never an event here.
 */

/** How often the ✋ reminds: never, on the backoff, or every 5 minutes (#6344). */
export const REMINDS = ["off", "gently", "often"] as const;
export type Remind = (typeof REMINDS)[number];

/** The minutes between reminders, from a reset: the last is repeated for ever ("Gently"). */
export const REMIND_AFTER_MINUTES = [5, 10, 20, 40, 60] as const;

/** The minutes between reminders at "Often": the backoff's first, fixed. */
export const OFTEN_MINUTES = REMIND_AFTER_MINUTES[0];

const MINUTE = 60_000;

/**
 * How long after reminder `step` (0 for the first after a reset) the next one is due, in ms, at
 * a pace that reminds at all.
 */
export function remindAfter(step: number, remind: Exclude<Remind, "off"> = "gently"): number {
  if (remind === "often") return OFTEN_MINUTES * MINUTE;
  const at = Math.min(step, REMIND_AFTER_MINUTES.length - 1);
  return REMIND_AFTER_MINUTES[at] * MINUTE;
}

/**
 * **What keeps the ✋ still**, read at the moment a bounce would play. `quiet` is the core's
 * Quiet, which is its schedule, Quiet now, and the person typing in a chat (`quiet.ts`).
 */
export type Holds = {
  /** Quiet: its schedule, Quiet now, or the person typing in a chat. */
  quiet: boolean;
  /** The system or purlis asks for less motion: the number just changes. */
  reduced: boolean;
  /** The purlis window is not in front, or this project is not the one in front. */
  behind: boolean;
};

/** Whether any hold keeps the ✋ still. */
export const held = (holds: Holds): boolean => holds.quiet || holds.reduced || holds.behind;

/** Where the schedule stands. */
export type Schedule = {
  /** Whether a Blocking ask waits in the project. */
  waiting: boolean;
  /** How many reminders have come due since the last reset. */
  step: number;
  /** When the next reminder is due, in ms since 1970, or `null` while nothing waits. */
  due: number | null;
  /** How many bounces have played: a higher number is a new bounce to play. */
  bounces: number;
  /** How often it reminds, as Settings › Attention › Remind me says. */
  remind: Remind;
};

/** Nothing waits, nothing has bounced, and it reminds gently. */
export const STILL: Schedule = { waiting: false, step: 0, due: null, bounces: 0, remind: "gently" };

/** What moves the schedule. `at` is the clock's time, in ms since 1970. */
export type BounceEvent =
  /** An ask arrived in the project: it bounces now, unless held, and the backoff restarts. */
  | { kind: "arrived"; at: number; holds: Holds }
  /** The person answered an ask. */
  | { kind: "answered"; at: number }
  /** The person opened Asks. */
  | { kind: "opened"; at: number }
  /** Whether a Blocking ask waits, as the project's asks were read. */
  | { kind: "waiting"; waiting: boolean; at: number }
  /** The timer: a reminder that has come due plays, unless held. */
  | { kind: "tick"; at: number; holds: Holds }
  /** The person changed Remind me: the new pace starts from now. */
  | { kind: "remind"; remind: Remind; at: number };

/** The backoff from its start, at `at`, with whatever waits; nothing is due at Off. */
const restarted = (s: Schedule, at: number): Schedule => ({
  ...s,
  step: 0,
  due: s.waiting && s.remind !== "off" ? at + remindAfter(0, s.remind) : null,
});

/** **The schedule after `event`.** Returns `s` itself when nothing changed. */
export function reduceBounce(s: Schedule, event: BounceEvent): Schedule {
  switch (event.kind) {
    case "arrived": {
      const bounces = held(event.holds) ? s.bounces : s.bounces + 1;
      return restarted({ ...s, waiting: true, bounces }, event.at);
    }
    case "answered":
    case "opened":
      return s.waiting ? restarted(s, event.at) : s;
    case "waiting":
      if (event.waiting === s.waiting) return s;
      return restarted({ ...s, waiting: event.waiting }, event.at);
    case "remind":
      if (event.remind === s.remind) return s;
      return restarted({ ...s, remind: event.remind }, event.at);
    case "tick": {
      if (s.due === null || s.remind === "off" || event.at < s.due) return s;
      const pace = s.remind;
      const step = s.step + 1;
      // From when it was due, so a late timer neither drifts the schedule nor makes one up.
      let due = s.due + remindAfter(step, pace);
      let next = step;
      while (due <= event.at) {
        next += 1;
        due += remindAfter(next, pace);
      }
      return {
        ...s,
        step: next,
        due,
        bounces: held(event.holds) ? s.bounces : s.bounces + 1,
      };
    }
  }
}
