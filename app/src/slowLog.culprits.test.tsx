// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/react";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { commands, type SlowMoment } from "./bindings";
import { timed } from "./answerWatch";
import { listen } from "./here";
import { Measured, watchForStalls } from "./slowLog";

/**
 * **A stall names what the window's thread was doing in it** (felt speed round 4, H9): the
 * React commits of that frame by the region that drew them, the core's events and answers the
 * window handled in it, and the scripts the webview says ran long, the top five of each.
 */

let written: SlowMoment[];

beforeEach(() => {
  written = [];
  vi.spyOn(commands, "recordSlow").mockImplementation((moment) => {
    written.push(moment);
    return Promise.resolve({ status: "ok", data: null });
  });
});

afterEach(() => {
  vi.restoreAllMocks();
});

/** Holds the thread for `ms` of real time, as a slow component does. */
function busy(ms: number): void {
  const until = performance.now() + ms;
  while (performance.now() < until) {
    // Spinning on purpose: the commit's time is what is measured.
  }
}

function Slow({ ms }: { ms: number }) {
  busy(ms);
  return null;
}

/** Frames the test hands out itself, at the times it says. */
function frames() {
  let next: ((time: number) => void) | undefined;
  const page = Object.assign(new EventTarget(), {
    visibilityState: "visible" as DocumentVisibilityState,
  });
  const stop = watchForStalls((callback) => {
    next = callback;
  }, page);
  return { at: (time: number) => next?.(time), stop };
}

function stall(): Extract<SlowMoment, { kind: "stall" }> {
  expect(written.map((one) => one.kind)).toEqual(["stall"]);
  return written[0] as Extract<SlowMoment, { kind: "stall" }>;
}

describe("a stall's React commits", () => {
  it("name the regions whose commits took longest in that frame, longest first", () => {
    const window = frames();
    window.at(1_000);
    render(
      <>
        <Measured id="ChatView">
          <Slow ms={2} />
        </Measured>
        <Measured id="PlaneView">
          <Slow ms={60} />
        </Measured>
      </>,
    );
    window.at(1_400);
    window.stop();

    const { react } = stall();
    expect(react.map((one) => one.name)).toEqual(["PlaneView", "ChatView"]);
    expect(react[0].ms).toBeGreaterThanOrEqual(60);
    expect(react[1].ms).toBeGreaterThanOrEqual(2);
  });
});

describe("a stall's events and answers", () => {
  afterEach(() => clearMocks());

  it("count the core's events the window handled in that frame, by name, most first", async () => {
    mockIPC(() => null, { shouldMockEvents: true });
    const stops = await Promise.all([
      listen("chat-events", () => {}),
      listen("plane-changed", () => {}),
    ]);
    const window = frames();
    window.at(1_000);
    await emit("chat-events", {});
    await emit("plane-changed", {});
    await emit("chat-events", {});
    window.at(1_300);
    window.stop();
    for (const stop of stops) stop();

    expect(stall().events).toEqual([
      { name: "chat-events", n: 2 },
      { name: "plane-changed", n: 1 },
    ]);
  });

  it("count the answers the window handled in that frame, by command", async () => {
    const window = frames();
    window.at(1_000);
    const ask = timed("alertsEverywhere", () => Promise.resolve());
    await ask();
    await ask();
    await timed("workspaceSaving", () => Promise.resolve())();
    window.at(1_300);
    window.stop();

    expect(stall().answers).toEqual([
      { name: "alertsEverywhere", n: 2 },
      { name: "workspaceSaving", n: 1 },
    ]);
  });

  it("name only the five most of each, and nothing a frame before the stall handled", async () => {
    const window = frames();
    window.at(1_000);
    await timed("before", () => Promise.resolve())();
    window.at(1_016);
    for (const [name, times] of [
      ["a", 6],
      ["b", 5],
      ["c", 4],
      ["d", 3],
      ["e", 2],
      ["f", 1],
    ] as const) {
      for (let i = 0; i < times; i++) await timed(name, () => Promise.resolve())();
    }
    window.at(1_400);
    window.stop();

    expect(stall().answers.map((one) => one.name)).toEqual(["a", "b", "c", "d", "e"]);
  });
});

describe("a stall's scripts", () => {
  it("are the scripts the webview said made the stalled frame long, by time, where it says", () => {
    let next: ((time: number) => void) | undefined;
    const page = Object.assign(new EventTarget(), {
      visibilityState: "visible" as DocumentVisibilityState,
    });
    const script = (startTime: number, duration: number, fn: string, invoker = "") => ({
      startTime,
      duration,
      invoker,
      sourceFunctionName: fn,
      sourceURL: "tauri://localhost/assets/index-abc123.js",
    });
    let pending: { startTime: number; duration: number; scripts: ReturnType<typeof script>[] }[] =
      [];
    const stop = watchForStalls(
      (callback) => {
        next = callback;
      },
      page,
      () => pending.splice(0),
    );
    next?.(1_000);
    pending = [
      // A frame before the stall's: not in it.
      { startTime: 900, duration: 60, scripts: [script(900, 60, "earlier")] },
      {
        startTime: 1_010,
        duration: 600,
        scripts: [
          script(1_010, 120, "flushBatch"),
          script(1_140, 400, "commitRoot"),
          script(1_550, 30, "", "TimerHandler:setTimeout"),
          script(1_590, 10, "flushBatch"),
        ],
      },
    ];
    next?.(1_620);
    stop();

    expect(stall().scripts).toEqual([
      { name: "commitRoot (index-abc123.js)", ms: 400 },
      { name: "flushBatch (index-abc123.js)", ms: 130 },
      { name: "TimerHandler:setTimeout", ms: 30 },
    ]);
  });
});

describe("a dark stall", () => {
  it("is marked dark when the thread answered all through the gap: its frames were withheld, as on a locked screen", async () => {
    const window = frames();
    window.at(performance.now());
    await new Promise((done) => setTimeout(done, 450));
    window.at(performance.now());
    window.stop();

    expect(stall()).toMatchObject({ dark: true });
  });

  it("is not marked dark when the thread was busy for the gap", () => {
    const window = frames();
    window.at(performance.now());
    busy(450);
    window.at(performance.now());
    window.stop();

    expect(stall()).toMatchObject({ dark: false });
  });
});
