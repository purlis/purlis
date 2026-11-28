// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { commands, type SlowMoment } from "./bindings";
import { MAIN } from "./here";
import { LATE_MS, forgetLateAnswers, timed, useLateAnswers } from "./answerWatch";
import { countsHotChats, frontTabIs, recentSlowLog, viewOpening, watchForStalls } from "./slowLog";

/**
 * **A slow moment is written to the log, and only a slow one** (`slowLog.ts`, #4725): an answer
 * of a second or more, a stall of the window's thread of 200 ms or more.
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
  vi.useRealTimers();
  vi.restoreAllMocks();
  act(() => forgetLateAnswers());
});

describe("an answer", () => {
  it("held past a second is written with its command, how long, when it was asked and the hot chats", async () => {
    vi.useFakeTimers({ now: 1_793_000_000_000 });
    const hot = countsHotChats(() => 3);
    let answer: () => void = () => {};
    const ask = timed("plane_sidebar", () => new Promise<void>((done) => (answer = done)));

    const asked = ask("/p");
    await act(async () => {
      vi.advanceTimersByTime(1_250);
      answer();
      await asked;
    });
    hot();

    expect(written).toEqual([
      { kind: "answer", command: "plane_sidebar", ms: 1_250, at: 1_793_000_000_000, hot: 3 },
    ]);
  });

  it("that comes within a second is not written", async () => {
    vi.useFakeTimers();
    let answer: () => void = () => {};
    const ask = timed("plane_sidebar", () => new Promise<void>((done) => (answer = done)));

    const asked = ask("/p");
    await act(async () => {
      vi.advanceTimersByTime(999);
      answer();
      await asked;
    });

    expect(written).toEqual([]);
  });

  it("is still said late in the window only at 20 s, and written once it comes", async () => {
    vi.useFakeTimers();
    let answer: () => void = () => {};
    const ask = timed("plane_sidebar", () => new Promise<void>((done) => (answer = done)));
    const { result } = renderHook(() => useLateAnswers());
    const said = () => result.current.map((one) => one.command);

    const asked = ask("/p");
    act(() => vi.advanceTimersByTime(LATE_MS - 1));
    expect(said()).toEqual([]);
    expect(written).toEqual([]);
    await act(async () => {
      vi.advanceTimersByTime(1);
      answer();
      await asked;
    });

    expect(said()).toEqual(["plane_sidebar"]);
    expect(written.map((one) => one.kind === "answer" && one.ms)).toEqual([LATE_MS]);
  });
});

/**
 * Frames a test hands out itself, at the times it says, to a page it hides and shows as a
 * browser does: by its visibility and a `visibilitychange`, with no frame sent while hidden.
 */
function frames() {
  let next: ((time: number) => void) | undefined;
  const page = Object.assign(new EventTarget(), {
    visibilityState: "visible" as DocumentVisibilityState,
  });
  const becomes = (state: DocumentVisibilityState) => {
    page.visibilityState = state;
    page.dispatchEvent(new Event("visibilitychange"));
  };
  const stop = watchForStalls((callback) => {
    next = callback;
  }, page);
  return {
    at: (time: number) => next?.(time),
    hide: () => becomes("hidden"),
    show: () => becomes("visible"),
    stop,
  };
}

describe("a stall", () => {
  it("of 200 ms or more between frames is written with the hot chats, the tab in front and a view opening", () => {
    vi.useFakeTimers({ now: 1_793_000_000_000 });
    const hot = countsHotChats(() => 12);
    const front = frontTabIs("steward 3");
    const opened = viewOpening();
    const window = frames();

    window.at(1_000);
    window.at(1_016);
    window.at(1_356);
    window.stop();
    opened();
    front();
    hot();

    expect(stalls()).toEqual([
      {
        kind: "stall",
        ms: 340,
        at: 1_793_000_000_000,
        window: MAIN,
        hot: 12,
        front: "steward 3",
        opening: true,
        // The test's frames are not the thread's: whether it was dark is the culprits' test's.
        dark: expect.any(Boolean),
        render_ms: 0,
        react: [],
        events: [],
        listeners: [],
        answers: [],
        marks: [],
      },
    ]);
  });

  it("is not a frame that came within 200 ms", () => {
    const window = frames();
    window.at(1_000);
    window.at(1_199);
    window.stop();
    expect(stalls()).toEqual([]);
  });

  it("is not the gap while the window was hidden, which is sent no frames", () => {
    const window = frames();
    window.at(1_000);
    window.hide();
    window.show();
    window.at(9_000);
    window.at(9_016);
    window.stop();
    expect(stalls()).toEqual([]);
  });

  it("is not measured from a frame a hidden window was sent after all", () => {
    const window = frames();
    window.at(1_000);
    window.hide();
    window.at(1_500);
    window.show();
    window.at(9_000);
    window.at(9_016);
    window.stop();
    expect(stalls()).toEqual([]);
  });

  it("says no view was opening once its opening is over, and no tab when none is in front", () => {
    const opened = viewOpening();
    opened();
    opened();
    const window = frames();
    window.at(0);
    window.at(500);
    window.stop();
    expect(stalls()).toMatchObject([{ kind: "stall", front: null, opening: false, hot: 0 }]);
  });
});

/** The stalls written, without the visibility changes beside them. */
function stalls(): SlowMoment[] {
  return written.filter((one) => one.kind === "stall");
}

describe("the window's visibility", () => {
  it("is written as stalls start being watched, then each time the window is hidden or shown, and nothing while it stays as it was", () => {
    vi.useFakeTimers({ now: 1_793_000_000_000 });
    const window = frames();
    vi.advanceTimersByTime(1_000);
    window.hide();
    vi.advanceTimersByTime(5_000);
    window.show();
    window.show();
    window.stop();

    expect(written).toEqual([
      { kind: "visibility", at: 1_793_000_000_000, window: MAIN, visible: true },
      { kind: "visibility", at: 1_793_000_001_000, window: MAIN, visible: false },
      { kind: "visibility", at: 1_793_000_006_000, window: MAIN, visible: true },
    ]);
  });

  it("and the stalls name the window they were seen in by its label, so a split window's rows can be told apart", () => {
    const internals = globalThis as { __TAURI_INTERNALS__?: unknown };
    const was = internals.__TAURI_INTERNALS__;
    internals.__TAURI_INTERNALS__ = { metadata: { currentWindow: { label: "plane-2" } } };
    try {
      const window = frames();
      window.at(1_000);
      window.at(1_500);
      window.hide();
      window.stop();
    } finally {
      internals.__TAURI_INTERNALS__ = was;
    }

    expect(written.map((one) => [one.kind, "window" in one ? one.window : undefined])).toEqual([
      ["visibility", "plane-2"],
      ["stall", "plane-2"],
      ["visibility", "plane-2"],
    ]);
  });

  it("is not written once stalls are no longer watched", () => {
    const window = frames();
    window.stop();
    window.hide();
    expect(written).toMatchObject([{ kind: "visibility", visible: true }]);
  });
});

describe("the recent entries", () => {
  it("are the core's answer, newest last, and nothing when it could not read the log", async () => {
    const entries: SlowMoment[] = [
      { kind: "answer", command: "plane_sidebar", ms: 1_250, at: 1, hot: 3 },
      {
        kind: "stall",
        ms: 340,
        at: 2,
        window: MAIN,
        hot: 12,
        front: "steward 3",
        opening: true,
        dark: false,
        render_ms: 210,
        react: [{ name: "PlaneView", ms: 210, self_ms: 210, commits: 1 }],
        events: [{ name: "chat-events", n: 4 }],
        listeners: [{ name: "chat-events", n: 4 }],
        answers: [],
        marks: [],
      },
      { kind: "visibility", at: 3, window: MAIN, visible: false },
    ];
    vi.spyOn(commands, "slowLogRecent").mockResolvedValueOnce({ status: "ok", data: entries });
    expect(await recentSlowLog()).toEqual(entries);

    vi.spyOn(commands, "slowLogRecent").mockResolvedValueOnce({ status: "error", error: "no" });
    expect(await recentSlowLog()).toBeNull();
  });
});
