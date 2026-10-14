// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { commands } from "./bindings";
import { KEPT, said, startTheSwitchClock, switched, type Switched } from "./switchClock";

/**
 * **The window times its own project switches** (`switchClock.ts`), so the operator's machine can
 * say what a test cannot: how long a switch took to be drawn and painted there, and how long the
 * core's synchronous commands took to answer for it.
 */

/** The switches the clock holds, read as a view of the doctor's dialog reads them. */
async function held(): Promise<readonly Switched[]> {
  const { renderHook } = await import("@testing-library/react");
  const { useSwitches } = await import("./switchClock");
  return renderHook(() => useSwitches()).result.current;
}

beforeEach(() => {
  vi.useFakeTimers();
  // A window that has been up a while: a press has a time of its own, after the window's start.
  vi.advanceTimersByTime(10_000);
});
afterEach(() => {
  clearMocks();
  vi.useRealTimers();
});

/** A press `ago` milliseconds back, as the engine stamps one. */
const pressed = (ago: number) => ({ timeStamp: performance.now() - ago }) as Event;

describe("a switch", () => {
  it("is timed from the press that made it to the window having drawn it, and to the paint", async () => {
    switched("/home/dev/one", pressed(40));
    const [one] = await held();
    expect(one.to).toBe("/home/dev/one");
    expect(one.drawnMs).toBeCloseTo(40, 0);
    expect(one.paintedMs).toBeUndefined();

    // The frame, then the task after it.
    vi.advanceTimersByTime(20);
    expect((await held())[0].paintedMs).toBeGreaterThanOrEqual(56);
  });

  it("has no press to be timed from where nothing pressed, and is still timed to its paint", async () => {
    switched("/home/dev/one", undefined);
    vi.advanceTimersByTime(20);
    const [one] = await held();
    expect(one.drawnMs).toBeUndefined();
    expect(one.paintedMs).toBeGreaterThanOrEqual(16);
  });

  it("is not timed from something the person did long before it", async () => {
    switched("/home/dev/one", pressed(60_000));
    expect((await held())[0].drawnMs).toBeUndefined();
  });

  it("is kept with the last few, the oldest let go", async () => {
    for (let n = 0; n < KEPT + 3; n++) {
      switched(`/home/dev/p${n}`, undefined);
      vi.advanceTimersByTime(10);
    }
    const all = await held();
    expect(all).toHaveLength(KEPT);
    expect(all[0].to).toBe("/home/dev/p3");
  });
});

describe("the core's synchronous commands", () => {
  it("are timed from the ask to the answer, and counted for the switch they were asked in", async () => {
    mockIPC(
      (cmd) =>
        new Promise((answer) => setTimeout(() => answer(null), cmd === "sandbox_state" ? 30 : 5)),
    );
    startTheSwitchClock();
    switched("/home/dev/one", pressed(10));

    const before = commands.sandboxState("/home/dev/one");
    await vi.advanceTimersByTimeAsync(40);
    await before;
    // Painted by now: what is asked from here is asked behind the project on screen.
    const after = [commands.sandboxState("/home/dev/one"), commands.sandboxState("/home/dev/one")];
    await vi.advanceTimersByTimeAsync(40);
    await Promise.all(after);

    const [one] = await held();
    expect(one.asked.map((asked) => [asked.command, asked.afterPaint])).toEqual([
      ["sandbox_state", false],
      ["sandbox_state", true],
      ["sandbox_state", true],
    ]);
    for (const asked of one.asked) expect(asked.ms).toBeGreaterThanOrEqual(30);
  });

  it("are not counted for a switch long over, nor measured: a gauge asks for as long as the window is up", async () => {
    mockIPC(() => null);
    startTheSwitchClock();
    switched("/home/dev/one", undefined);
    await vi.advanceTimersByTimeAsync(5_000);
    const measured = vi.spyOn(performance, "measure");
    for (let n = 0; n < 50; n++) await commands.chatUsage("/home/dev/one", 1);
    expect((await held())[0].asked).toEqual([]);
    expect(measured).not.toHaveBeenCalled();
    measured.mockRestore();
  });
});

describe("a switch, as the doctor's dialog says it", () => {
  it("says the project, both times, and each command's answers, before the paint apart from after", () => {
    const line = said(
      {
        to: "/home/dev/volaticloud",
        at: new Date(2026, 9, 14, 9, 5, 7).getTime(),
        drawnMs: 38.4,
        paintedMs: 61,
        asked: [
          { command: "inbox_shown", ms: 1.2, afterPaint: false },
          { command: "sandbox_state", ms: 19, afterPaint: true },
          { command: "sandbox_state", ms: 33, afterPaint: true },
        ],
      },
      (plane) => plane.split("/").at(-1) ?? plane,
    );
    expect(line).toBe(
      "09:05:07 to volaticloud: drawn 38 ms, painted 61 ms; the core's synchronous commands " +
        "before the paint: inbox_shown ×1 1 ms; after it: sandbox_state ×2 52 ms (longest 33)",
    );
  });

  it("says so where a time was not taken and nothing was asked", () => {
    const line = said({ to: "p", at: 0, drawnMs: undefined, paintedMs: undefined, asked: [] });
    expect(line).toContain("drawn not timed, painted not timed");
    expect(line).toContain("before the paint: none; after it: none");
  });
});
