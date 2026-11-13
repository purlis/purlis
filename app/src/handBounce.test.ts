// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  type BounceEvent,
  type Holds,
  reduceBounce,
  remindAfter,
  type Schedule,
  STILL,
} from "./handBounce";

/**
 * **The ✋'s reminder bounce, as a schedule** (#6343, ADR 0121 amended 2026-11-12): driven by a
 * fake clock, minute by minute, as the timer would drive it.
 */

const MINUTE = 60_000;
const FREE: Holds = { quiet: false, reduced: false, behind: false };

/** The schedule after each event in turn. */
const after = (events: readonly BounceEvent[], from: Schedule = STILL): Schedule =>
  events.reduce(reduceBounce, from);

/** The minutes, up to `until`, at which a bounce plays: the timer ticking every minute. */
function bouncesUntil(
  start: Schedule,
  until: number,
  holdsAt: (minute: number) => Holds = () => FREE,
  also: (minute: number) => BounceEvent[] = () => [],
): number[] {
  const played: number[] = [];
  let s = start;
  for (let minute = 1; minute <= until; minute += 1) {
    const at = minute * MINUTE;
    const before = s.bounces;
    s = after([...also(minute), { kind: "tick", at, holds: holdsAt(minute) }], s);
    if (s.bounces > before) played.push(minute);
  }
  return played;
}

/** One Blocking ask arrived at minute 0, with nothing holding. */
const oneWaits = () => after([{ kind: "arrived", at: 0, holds: FREE }]);

describe("the backoff (5, 10, 20, 40, then every 60 minutes)", () => {
  it("waits 5, 10, 20, 40 minutes, then 60 for ever", () => {
    expect([0, 1, 2, 3, 4, 5, 9].map((step) => remindAfter(step) / MINUTE)).toEqual([
      5, 10, 20, 40, 60, 60, 60,
    ]);
  });

  it("bounces at 5, 15, 35, 75, 135 and 195 minutes while one Blocking ask waits", () => {
    expect(bouncesUntil(oneWaits(), 200)).toEqual([5, 15, 35, 75, 135, 195]);
  });

  it("bounces as an ask arrives, before any reminder", () => {
    expect(oneWaits().bounces).toBe(1);
  });

  it("starts the backoff when asks are first read with one waiting, without a bounce", () => {
    const s = after([{ kind: "waiting", waiting: true, at: 0 }]);
    expect(s.bounces).toBe(0);
    expect(bouncesUntil(s, 40)).toEqual([5, 15, 35]);
  });
});

describe("what restarts the backoff at 5 minutes", () => {
  const restartsAt = (event: (at: number) => BounceEvent) =>
    bouncesUntil(oneWaits(), 60, undefined, (minute) =>
      minute === 12 ? [event(minute * MINUTE)] : [],
    );

  it("a new ask, which bounces as it arrives", () => {
    const played = bouncesUntil(oneWaits(), 60, undefined, (minute) =>
      minute === 12 ? [{ kind: "arrived", at: minute * MINUTE, holds: FREE }] : [],
    );
    expect(played).toEqual([5, 12, 17, 27, 47]);
  });

  it("an answer", () => {
    expect(restartsAt((at) => ({ kind: "answered", at }))).toEqual([5, 17, 27, 47]);
  });

  it("Asks opening", () => {
    expect(restartsAt((at) => ({ kind: "opened", at }))).toEqual([5, 17, 27, 47]);
  });
});

describe("what stops it", () => {
  it("nothing waiting: no reminder, and an answer or Asks opening does not start one", () => {
    const stopped = after([{ kind: "waiting", waiting: false, at: 7 * MINUTE }], oneWaits());
    expect(stopped.due).toBeNull();
    const poked = after(
      [
        { kind: "answered", at: 8 * MINUTE },
        { kind: "opened", at: 9 * MINUTE },
      ],
      stopped,
    );
    expect(poked).toBe(stopped);
    expect(bouncesUntil(poked, 200)).toEqual([]);
  });

  it("starts again from 5 minutes when something waits once more", () => {
    const stopped = after([{ kind: "waiting", waiting: false, at: 7 * MINUTE }], oneWaits());
    const again = after([{ kind: "waiting", waiting: true, at: 20 * MINUTE }], stopped);
    expect(bouncesUntil(again, 40)).toEqual([25, 35]);
  });
});

describe("the holds, with nothing made up after", () => {
  const holding = (hold: Partial<Holds>) => (minute: number) =>
    minute >= 10 && minute < 50 ? { ...FREE, ...hold } : FREE;

  it.each([
    ["Quiet (its schedule, Quiet now, or typing in a chat)", { quiet: true }],
    ["reduced motion", { reduced: true }],
    ["the window behind", { behind: true }],
  ])("holds the reminders due during %s, and goes on as scheduled after", (_, hold) => {
    // Due at 15, 35 (held) and 75: the next after the hold follows the schedule, none made up.
    expect(bouncesUntil(oneWaits(), 140, holding(hold))).toEqual([5, 75, 135]);
  });

  it("holds the bounce of an ask that arrives while held, and still restarts the backoff", () => {
    const s = after([{ kind: "arrived", at: 0, holds: { ...FREE, quiet: true } }]);
    expect(s.bounces).toBe(0);
    expect(bouncesUntil(s, 20)).toEqual([5, 15]);
  });

  it("plays one reminder for a timer that comes late, never the ones it missed", () => {
    // Asleep from minute 4 to minute 100: the reminders due at 5, 15, 35 and 75 were missed.
    const late = after([{ kind: "tick", at: 100 * MINUTE, holds: FREE }], oneWaits());
    expect(late.bounces).toBe(2);
    expect(bouncesUntil(late, 200).filter((minute) => minute > 100)).toEqual([135, 195]);
  });
});
