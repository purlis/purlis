// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Sideways wheel traces of a trackpad swipe** (#3327, R112): the `deltaX` of each event, one a
 * frame (16 ms apart), in the shape a macOS trackpad gives a two-finger swipe in WebKit. A stroke
 * ramps up while the fingers move. Its momentum then decays, a frame at a time, for up to a
 * second after they lift, with repeats and the odd one-pixel uptick from rounding. The DOM says
 * nothing of which events are momentum, so a swipe is read from these numbers alone.
 *
 * They are modelled on that shape, not captured from a device; the tests that feed them hold
 * what the gesture does with each part of it. A real trace is also not always one event a frame:
 * WebKit merges the wheel events of a busy page into one, their travel summed ({@link merged}),
 * and a workspace switch is when the page is busy.
 */

/** The fingers moving: from a touch to the swipe's fastest. Its travel passes 60px by the 6th. */
export const STROKE: readonly number[] = [1, 3, 6, 11, 17, 24, 30, 34, 36];

/** The momentum after {@link STROKE}: 48 frames (0.8 s), far more than another 60px of travel. */
export const MOMENTUM: readonly number[] = [
  35, 33, 31, 29, 27, 25, 23, 22, 20, 19, 17, 16, 15, 14, 13, 12, 11, 10, 9, 9, 8, 7, 7, 6, 6, 5, 5,
  4, 5, 4, 4, 3, 3, 3, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1,
];

/** One frame of a trackpad, in ms. */
export const FRAME = 16;

/** A wheel event as a trace gives it: its travel and when it came. */
export type Traced = { dx: number; at: number };

/**
 * **Traces laid end to end**: each part's events a frame apart, starting at `from`, and `gap` ms
 * between one part and the next (the fingers touching down again).
 */
export function played(parts: readonly (readonly number[])[], from = 0, gap = FRAME): Traced[] {
  const out: Traced[] = [];
  let at = from;
  parts.forEach((part, n) => {
    if (n > 0) at += gap - FRAME;
    for (const dx of part) {
      out.push({ dx, at });
      at += FRAME;
    }
  });
  return out;
}

/** The trace the other way: a swipe to the left. */
export const leftward = (part: readonly number[]): number[] => part.map((dx) => -dx);

/**
 * **A trace with `count` events from `start` merged into one**, as WebKit delivers them while
 * the page is busy: their travel summed, arriving when the last of them would have.
 */
export function merged(trace: readonly Traced[], start: number, count: number): Traced[] {
  const into = trace.slice(start, start + count);
  const one = { dx: into.reduce((sum, { dx }) => sum + dx, 0), at: into[into.length - 1].at };
  return [...trace.slice(0, start), one, ...trace.slice(start + count)];
}
