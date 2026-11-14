// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterPaint } from "./projectShown";

/**
 * **Work told while the window is not being drawn waits until it is, then runs once, after the
 * paint** (H11, the black window after time away).
 *
 * A window is away when it is hidden, and also when it stays `visible` with the display asleep or
 * the screen locked: the engine then draws one frame each 0.6 to 0.9 s, or none for over a minute
 * (our stall log of 2026-11-13), and may hold the page's tasks back, while the core goes on
 * telling it what its plane did. A reader that read the plane again for every change meanwhile
 * piled up hundreds of reads and commits, to be run as the person came back and ahead of the
 * window's first frame: a black window for tens of seconds.
 *
 * So {@link whenDrawing} runs its work at once in a window being drawn as it should be, and
 * otherwise owes it: while the window is hidden, while its frames come {@link DRAWING_GAP_MS} or
 * more apart, or while none has come for {@link STOPPED_MS}. Owed work runs once, however many
 * times it was asked for, when two frames come less than {@link DRAWING_GAP_MS} apart again, and
 * after that frame is painted (`afterPaint`): the window is drawn first, then catches up.
 * Everything owed runs in that one task, so what it sets is one commit and the reads it starts go
 * out together.
 *
 * The frames are the signal, and not a notice of the screen locking or the display sleeping:
 * nothing else says the engine has stopped drawing, and the frames say it whatever the cause.
 * They are watched for the life of the window ({@link watchTheFrames}, started in `main.tsx`);
 * where nothing watches them (a unit test), nothing says the window is away, and the work runs
 * at once as it always did.
 */

/** Frames this far apart or more are a window not being drawn as it should be. */
export const DRAWING_GAP_MS = 400;

/** No frame for this long is a window whose frames have stopped. */
export const STOPPED_MS = 1_000;

/** When the last frame came, and how long after the one before it; `undefined` while nothing
 *  watches the frames. */
let clock: { at: number | undefined; gap: number } | undefined;

const owed = new Set<() => void>();

const hidden = (): boolean => typeof document !== "undefined" && document.hidden;

/** Whether the window is being drawn as it should be, as far as its frames say. */
function drawn(): boolean {
  if (clock === undefined) return true;
  // No frame yet since the watch began or the window was shown again: not drawn yet.
  if (hidden() || clock.at === undefined) return false;
  return clock.gap < DRAWING_GAP_MS && performance.now() - clock.at < STOPPED_MS;
}

function catchUp(): void {
  if (owed.size === 0 || !drawn()) return;
  const now = [...owed];
  owed.clear();
  for (const run of now) afterPaint(run);
}

/**
 * Watches the window's frames for its life, so {@link whenDrawing} can tell a window drawn from
 * one away. Answers how to stop watching (a test's). Watching twice is watching once.
 */
export function watchTheFrames(): () => void {
  if (clock !== undefined) return stopWatching;
  const mine = { at: undefined as number | undefined, gap: 0 };
  clock = mine;
  // No frame comes while the window is hidden, so the frame before a hide is not what the first
  // one after it is measured from, as the stall log measures them (`slowLog.ts`).
  const startAgain = () => {
    mine.at = undefined;
    mine.gap = 0;
  };
  document.addEventListener("visibilitychange", startAgain);
  const onFrame = () => {
    if (clock !== mine) return;
    const now = performance.now();
    mine.gap = mine.at === undefined ? 0 : now - mine.at;
    mine.at = now;
    catchUp();
    requestAnimationFrame(onFrame);
  };
  requestAnimationFrame(onFrame);
  stopWatching = () => {
    if (clock !== mine) return;
    clock = undefined;
    document.removeEventListener("visibilitychange", startAgain);
  };
  return stopWatching;
}

let stopWatching = () => {};

/**
 * Runs `run` now if the window is being drawn, or else once it is drawn again, after the paint:
 * once, however many times it is asked for before then. Answers how to call it off.
 */
export function whenDrawing(run: () => void): () => void {
  if (drawn()) {
    run();
    return () => {};
  }
  owed.add(run);
  return () => {
    owed.delete(run);
  };
}
