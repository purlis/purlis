// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Channel } from "@tauri-apps/api/core";
import type { Terminal } from "@xterm/xterm";
import { SessionPane } from "./SessionPane";

/**
 * **One terminal write per pane per frame** (#4732): a long harness message arrives as many
 * chunks, and every chunk that arrives within a frame reaches the terminal as one write, in
 * the order it came.
 *
 * The real xterm, wrapped only so the test can reach the instance the pane made and count what
 * it is asked to write. Frames are the test's: `requestAnimationFrame` queues, and `frame()`
 * runs what is queued, xterm's own render requests included.
 */
const made: Terminal[] = [];
vi.mock("@xterm/xterm", async (real) => {
  const xterm = await real<typeof import("@xterm/xterm")>();
  class Recorded extends xterm.Terminal {
    constructor(...args: ConstructorParameters<typeof xterm.Terminal>) {
      super(...args);
      made.push(this);
    }
  }
  return { ...xterm, Terminal: Recorded };
});

// The renderer loads WebGL code jsdom has no context for; the pane draws with the DOM meanwhile.
vi.mock("./renderer", async (real) => ({
  ...(await real<typeof import("./renderer")>()),
  draw: () => new Promise(() => {}),
}));

// jsdom has no `matchMedia`, and xterm asks it for the display's pixel ratio as it opens.
vi.stubGlobal("matchMedia", (query: string) => ({
  matches: false,
  media: query,
  addEventListener: () => {},
  removeEventListener: () => {},
  addListener: () => {},
  removeListener: () => {},
}));

const ownFrame = globalThis.requestAnimationFrame;
const ownCancel = globalThis.cancelAnimationFrame;
let queued = new Map<number, FrameRequestCallback>();
let nextId = 0;
/** Runs one frame: whatever was asked for before it. */
const frame = () =>
  act(() => {
    const due = queued;
    queued = new Map();
    for (const then of due.values()) then(performance.now());
  });

/** The view's output channel, as the pane handed it to the core. */
let output: Channel<string> | undefined;
let index = 0;
/** One chunk from the core, delivered as Tauri delivers a channel's message. */
const send = (message: string) =>
  act(() => {
    if (!output) throw new Error("the pane has opened no view");
    window.__TAURI_INTERNALS__.runCallback(output.id, { index: index++, message });
  });

beforeEach(() => {
  made.length = 0;
  output = undefined;
  index = 0;
  queued = new Map();
  vi.stubGlobal("requestAnimationFrame", (then: FrameRequestCallback) => {
    nextId += 1;
    queued.set(nextId, then);
    return nextId;
  });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => queued.delete(id));
  mockIPC((cmd, args) => {
    if (cmd === "watch_session") {
      output = (args as { output: Channel<string> }).output;
      return { view: 1, columns: 80, rows: 24, scrollback: 5000, newline: null };
    }
    return null;
  });
});
afterEach(() => {
  cleanup();
  clearMocks();
  vi.stubGlobal("requestAnimationFrame", ownFrame);
  vi.stubGlobal("cancelAnimationFrame", ownCancel);
});

/** Renders a pane, waits until its view is open, and counts its terminal's writes from then. */
async function openPane() {
  const shown = render(
    <SessionPane plane="/planes/one" session={1} focused={false} onFocus={() => {}} />,
  );
  await waitFor(() => expect(made[0]?.options.scrollback).toBe(5000));
  const writes = vi.spyOn(made[0], "write");
  return { ...shown, writes };
}

describe("a chat's output reaching its terminal (#4732)", () => {
  it("writes the chunks that arrive within one frame as one write, in order", async () => {
    const { writes } = await openPane();

    await send("first ");
    await send("second ");
    await send("third");
    expect(writes).not.toHaveBeenCalled();

    frame();

    expect(writes).toHaveBeenCalledTimes(1);
    expect(writes.mock.calls[0][0]).toBe("first second third");
  });

  it("writes once more for the chunks of the next frame, and nothing for a frame with none", async () => {
    const { writes } = await openPane();

    await send("one");
    frame();
    frame();
    await send("two");
    await send("three");
    frame();

    expect(writes.mock.calls.map(([text]) => text)).toEqual(["one", "twothree"]);
  });

  it("writes nothing once the pane is gone, even for chunks it had gathered", async () => {
    const { writes, unmount } = await openPane();

    await send("late");
    unmount();
    frame();

    expect(writes).not.toHaveBeenCalled();
  });
});
