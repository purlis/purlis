// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/react";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { commands, type SlowMoment } from "./bindings";
import { timed } from "./answerWatch";
import { listen } from "./here";
import { Measured, marked, useMarkedOnChange, useMarkedOnMount, watchForStalls } from "./slowLog";

/**
 * **A stall names what the window's thread was doing in it** (felt speed round 4, H9; #7891), so
 * its causes can be told apart: how much of the gap React's renders took, the regions that
 * rendered by their own time and commits, the core's events apart from their listeners' calls,
 * the answers handled, and the marks of what costs a frame, the top five of each.
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

function Again({ ms, children }: { ms: number; children?: React.ReactNode }) {
  busy(ms);
  return children;
}

describe("a stall's React renders", () => {
  it("name the regions whose own renders took longest in that frame, longest first", () => {
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

  it("tell a region's own body apart from the regions inside it, so PlaneView's is its own", () => {
    const window = frames();
    window.at(1_000);
    render(
      <Measured id="PlaneView">
        <Again ms={20}>
          <Measured id="changes">
            <Slow ms={80} />
          </Measured>
        </Again>
      </Measured>,
    );
    window.at(1_400);
    window.stop();

    const { react } = stall();
    const [changes, plane] = react;
    expect(react.map((one) => one.name)).toEqual(["changes", "PlaneView"]);
    expect(changes.self_ms).toBe(changes.ms);
    expect(plane.ms).toBeGreaterThanOrEqual(changes.ms + 20);
    expect(plane.self_ms).toBeGreaterThanOrEqual(20);
    expect(plane.self_ms).toBeLessThan(changes.ms);
  });

  it("count each region's commits in that frame", () => {
    const window = frames();
    window.at(1_000);
    const drawn = render(
      <Measured id="PlaneView">
        <Slow ms={1} />
      </Measured>,
    );
    drawn.rerender(
      <Measured id="PlaneView">
        <Slow ms={2} />
      </Measured>,
    );
    drawn.rerender(
      <Measured id="PlaneView">
        <Slow ms={3} />
      </Measured>,
    );
    window.at(1_400);
    window.stop();

    expect(stall().react).toMatchObject([{ name: "PlaneView", commits: 3 }]);
  });

  it("say how much of the gap React's renders took: every region's own time, each counted once", () => {
    const window = frames();
    window.at(1_000);
    render(
      <>
        <Measured id="PlaneView">
          <Again ms={20}>
            <Measured id="changes">
              <Slow ms={60} />
            </Measured>
          </Again>
        </Measured>
        <Measured id="ChatView">
          <Slow ms={30} />
        </Measured>
      </>,
    );
    window.at(1_400);
    window.stop();

    const { react, render_ms, ms } = stall();
    const own = react.reduce((sum, one) => sum + one.self_ms, 0);
    expect(render_ms).toBeGreaterThanOrEqual(110);
    expect(Math.abs(render_ms - own)).toBeLessThanOrEqual(react.length);
    expect(render_ms).toBeLessThan(react.reduce((sum, one) => sum + one.ms, 0));
    expect(render_ms).toBeLessThanOrEqual(ms);
  });
});

describe("a stall's marks", () => {
  it("count what happened in that frame that costs one, by name, most first", () => {
    const window = frames();
    window.at(1_000);
    marked("tab-switch");
    marked("pane-mount");
    marked("pane-mount");
    window.at(1_300);
    window.stop();

    expect(stall().marks).toEqual([
      { name: "pane-mount", n: 2 },
      { name: "tab-switch", n: 1 },
    ]);
  });

  it("mark each mount of a component, and nothing as it is drawn again", () => {
    function Pane() {
      useMarkedOnMount("pane-mount");
      return null;
    }
    const window = frames();
    window.at(1_000);
    const drawn = render(
      <>
        <Pane />
        <Pane />
      </>,
    );
    drawn.rerender(
      <>
        <Pane />
        <Pane />
      </>,
    );
    window.at(1_300);
    window.stop();

    expect(stall().marks).toEqual([{ name: "pane-mount", n: 2 }]);
  });

  it("mark a change of what a component watches, never the first value it is drawn with", () => {
    function Front({ tab }: { tab: number | undefined }) {
      useMarkedOnChange("tab-switch", tab);
      return null;
    }
    const window = frames();
    window.at(1_000);
    const drawn = render(<Front tab={1} />);
    drawn.rerender(<Front tab={1} />);
    drawn.rerender(<Front tab={2} />);
    drawn.rerender(<Front tab={undefined} />);
    drawn.rerender(<Front tab={3} />);
    window.at(1_300);
    window.stop();

    expect(stall().marks).toEqual([{ name: "tab-switch", n: 2 }]);
  });
});

describe("a stall's events and answers", () => {
  afterEach(() => clearMocks());

  it("count the core's events the window handled in that frame, each once, by name, most first", async () => {
    mockIPC(() => null, { shouldMockEvents: true });
    const stops = await Promise.all([
      listen("chat-events", () => {}),
      listen("plane-changed", () => {}),
      listen("plane-changed", () => {}),
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

  it("count the calls of the window's listeners those events made apart, most first", async () => {
    mockIPC(() => null, { shouldMockEvents: true });
    const stops = await Promise.all([
      listen("chat-events", () => {}),
      listen("plane-changed", () => {}),
      listen("plane-changed", () => {}),
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

    expect(stall().listeners).toEqual([
      { name: "plane-changed", n: 3 },
      { name: "chat-events", n: 2 },
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
  it("are no longer named: WebKit, the webview the window runs in, never says them", () => {
    const window = frames();
    window.at(1_000);
    window.at(1_400);
    window.stop();

    expect(stall()).not.toHaveProperty("scripts");
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
