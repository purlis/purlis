// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DRAWING_GAP_MS, STOPPED_MS, watchTheFrames, whenDrawing } from "./whenDrawing";

/**
 * **Work asked for while the window is not drawn runs once it is drawn again, after the paint**
 * (H11): hidden, or `visible` with its frames slowed or stopped by a sleeping display or a locked
 * screen. The clock and the frames are moved by hand here, to the times each case names.
 */

let frames: FrameRequestCallback[];
let hidden: boolean;
let now: number;
let stop: () => void;

beforeEach(() => {
  frames = [];
  hidden = false;
  now = 0;
  vi.spyOn(document, "hidden", "get").mockImplementation(() => hidden);
  vi.spyOn(performance, "now").mockImplementation(() => now);
  vi.spyOn(window, "requestAnimationFrame").mockImplementation((then) => {
    frames.push(then);
    return frames.length;
  });
  vi.useFakeTimers({ toFake: ["setTimeout"] });
  stop = watchTheFrames();
});

afterEach(() => {
  stop();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

/** Draws a frame at `time`, as the engine would, and runs what was set to run after it. */
function drawAt(time: number): void {
  now = time;
  for (const then of frames.splice(0)) then(time);
  vi.runOnlyPendingTimers();
}

/** Frames as the window draws them, from `from`: `count` of them, 16 ms apart. */
function drawnFrom(from: number, count = 3): number {
  for (let n = 0; n < count; n++) drawAt(from + n * 16);
  return from + count * 16;
}

describe("work asked for of a window", () => {
  it("runs at once where nothing watches the frames, as it always did", () => {
    stop();
    hidden = true;
    const run = vi.fn();
    whenDrawing(run);
    expect(run).toHaveBeenCalledTimes(1);
  });

  it("runs at once in a window drawn as it should be", () => {
    drawnFrom(0);
    const run = vi.fn();
    whenDrawing(run);
    expect(run).toHaveBeenCalledTimes(1);
  });

  it("waits while the frames come slowly, as with the display asleep, then runs once, after the paint", () => {
    let at = drawnFrom(0);
    const run = vi.fn();
    for (let n = 0; n < 100; n++) {
      drawAt((at += 700));
      whenDrawing(run);
    }
    expect(run).not.toHaveBeenCalled();
    // The display wakes: frames come as they should again, and the window is drawn first.
    drawAt((at += 16));
    expect(run).not.toHaveBeenCalled();
    drawAt(at + 16);
    expect(run).toHaveBeenCalledTimes(1);
  });

  it("waits once no frame has come for a while, then runs once the window is drawn again", () => {
    const at = drawnFrom(0);
    now = at + STOPPED_MS;
    const run = vi.fn();
    for (let n = 0; n < 100; n++) whenDrawing(run);
    expect(run).not.toHaveBeenCalled();
    drawAt(at + 76_800);
    drawAt(at + 76_800 + DRAWING_GAP_MS - 1);
    drawAt(at + 76_800 + DRAWING_GAP_MS + 15);
    expect(run).toHaveBeenCalledTimes(1);
  });

  it("waits while the window is hidden, and runs once it is shown and drawn", () => {
    const at = drawnFrom(0);
    hidden = true;
    document.dispatchEvent(new Event("visibilitychange"));
    const run = vi.fn();
    for (let n = 0; n < 100; n++) whenDrawing(run);
    expect(run).not.toHaveBeenCalled();
    hidden = false;
    document.dispatchEvent(new Event("visibilitychange"));
    // Shown, before any frame: still waiting.
    whenDrawing(run);
    expect(run).not.toHaveBeenCalled();
    drawnFrom(at + 60_000);
    expect(run).toHaveBeenCalledTimes(1);
  });

  it("does not run once called off", () => {
    let at = drawnFrom(0);
    drawAt((at += 700));
    const run = vi.fn();
    whenDrawing(run)();
    drawnFrom(at + 16);
    expect(run).not.toHaveBeenCalled();
  });
});
