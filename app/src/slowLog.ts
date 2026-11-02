// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { commands, type SlowMoment } from "./bindings";

/**
 * **A slow moment the person felt is written down, so it can be read afterwards** (#4725).
 *
 * The window says an answer is late only at 20 s (`answerWatch.ts`), and a stall of its own
 * thread it never says at all. A second's wait for a tab, or half a second of a frozen window
 * while fifty chats work, is felt and then gone, with nothing left to look at. Each one is
 * written here, as one JSON line in `slow.jsonl` in the app's log folder, which the core keeps
 * to its newest 1,000 (`purlis_core::slowlog`): machine-local, never in a plane.
 *
 * - **An answer that took {@link SLOW_MS} or longer**, with its command, how long it took, when it
 *   was asked and how many chats were working. Every command `answerWatch` times, so the ones
 *   that wait on a person or the network by design (`WAITS_BY_DESIGN`) are left out here too.
 * - **A stall of {@link STALL_MS} or longer**: a frame that came that long after the one before
 *   it ({@link watchForStalls}), with how many chats were working, the tab in front, and whether
 *   a view of a chat was being opened.
 *
 * Only written: nothing in the window reads it back or says it. Writing it is the one command
 * that is not timed (`answerWatch.ts`), so a slow write is not a slow answer to write.
 */

/** An answer this long or longer is written down. */
export const SLOW_MS = 1_000;

/** A gap between frames this long or longer is a stall, and written down. */
export const STALL_MS = 200;

/** The command that writes a slow moment: never timed itself. */
export const WRITES_THE_LOG = "recordSlow";

/** What says how many chats one project holds are working, each counted as it is asked. */
const hotSources = new Set<() => number>();

/** The name of the tab in front of the project in front, as the person reads it. */
let front: { name: string | undefined } | undefined;

/** How many views of a chat are being opened now. */
let opening = 0;

/** How many chats are working in every project the window holds. */
export function hotChats(): number {
  let hot = 0;
  for (const count of hotSources) hot += count();
  return hot;
}

/** `count` says how many chats of one project are working, until the answer is called. */
export function countsHotChats(count: () => number): () => void {
  hotSources.add(count);
  return () => void hotSources.delete(count);
}

/** The tab named `name` is in front, until the answer is called (and nothing has replaced it). */
export function frontTabIs(name: string | undefined): () => void {
  const mine = { name };
  front = mine;
  return () => {
    if (front === mine) front = undefined;
  };
}

/** A view of a chat is being opened, until the answer is called. Calling it again does nothing. */
export function viewOpening(): () => void {
  opening += 1;
  let done = false;
  return () => {
    if (done) return;
    done = true;
    opening -= 1;
  };
}

function write(moment: SlowMoment): void {
  // A log that cannot be written is let go: it is a record, never a reason to stop the window.
  void commands.recordSlow(moment).catch(() => {});
}

/** `command` took `ms` to answer: written down when that is {@link SLOW_MS} or longer. */
export function answered(command: string, ms: number, askedAt: number): void {
  if (ms < SLOW_MS) return;
  write({ kind: "answer", command, ms: Math.round(ms), at: askedAt, hot: hotChats() });
}

/** What schedules the next frame, and what it hands the callback: the browser's own. */
type Frames = (next: (time: number) => void) => unknown;

let watching: (() => void) | undefined;

/**
 * Watches the window's frames for the life of the window: a frame that comes {@link STALL_MS} or
 * more after the one before it is a stall, and written down with what was going on.
 *
 * A window that is not visible is sent no frames on purpose, so the gap across a hide is not a
 * stall: the clock starts again with the first frame after the window is shown. Answers how to
 * stop watching (a test's).
 */
export function watchForStalls(
  frames: Frames = (next) => requestAnimationFrame(next),
  visible: () => boolean = () => document.visibilityState === "visible",
): () => void {
  if (watching) return watching;
  let last: number | undefined;
  let stopped = false;
  const onFrame = (time: number) => {
    if (stopped) return;
    if (!visible()) last = undefined;
    else {
      if (last !== undefined && time - last >= STALL_MS) {
        write({
          kind: "stall",
          ms: Math.round(time - last),
          at: Date.now(),
          hot: hotChats(),
          front: front?.name ?? null,
          opening: opening > 0,
        });
      }
      last = time;
    }
    frames(onFrame);
  };
  frames(onFrame);
  watching = () => {
    stopped = true;
    watching = undefined;
  };
  return watching;
}
