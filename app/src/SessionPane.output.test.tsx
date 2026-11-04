// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Channel } from "@tauri-apps/api/core";
import type { Terminal } from "@xterm/xterm";
import { SessionPane } from "./SessionPane";
import { letGoOfKeptTerminals } from "./keptTerminals";

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
let output: Channel<ArrayBuffer> | undefined;
let index = 0;
/**
 * One chunk from the core, delivered as Tauri delivers a channel's raw body: an `ArrayBuffer`
 * of the bytes the program wrote (#4745). Text is written as its UTF-8.
 */
const send = (chunk: string | Uint8Array) =>
  act(() => {
    if (!output) throw new Error("the pane has opened no view");
    const bytes = typeof chunk === "string" ? new TextEncoder().encode(chunk) : chunk;
    const message = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
    window.__TAURI_INTERNALS__.runCallback(output.id, { index: index++, message });
  });
/** What one terminal write carried, as text. */
const text = (written: unknown) => new TextDecoder().decode(written as Uint8Array);

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
      output = (args as { output: Channel<ArrayBuffer> }).output;
      return { view: 1, columns: 80, rows: 24, scrollback: 5000, newline: null };
    }
    return null;
  });
});
afterEach(() => {
  cleanup();
  // A chat shown last keeps its terminal (`keptTerminals.ts`), and letting go of it closes its
  // view through the core: let go while this file's IPC mock still answers.
  letGoOfKeptTerminals();
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
    expect(text(writes.mock.calls[0][0])).toBe("first second third");
  });

  it("writes once more for the chunks of the next frame, and nothing for a frame with none", async () => {
    const { writes } = await openPane();

    await send("one");
    frame();
    frame();
    await send("two");
    await send("three");
    frame();

    expect(writes.mock.calls.map(([written]) => text(written))).toEqual(["one", "twothree"]);
  });

  it("writes nothing once its terminal is let go of, even for chunks it had gathered", async () => {
    const { writes, unmount } = await openPane();

    await send("late");
    unmount();
    // The chat was shown last, so its terminal was kept (`keptTerminals.ts`) until now.
    letGoOfKeptTerminals();
    frame();

    expect(writes).not.toHaveBeenCalled();
  });
});

describe("a chat's output reaching its terminal as the bytes it is (#4745)", () => {
  it("writes the bytes the program wrote, unchanged, as one write per frame", async () => {
    const { writes } = await openPane();
    const wrote = [
      new Uint8Array([0x63, 0x61, 0x66, 0xc3]),
      new Uint8Array([0xa9, 0x1b, 0x5b, 0x6d]),
    ];

    await send(wrote[0]);
    await send(wrote[1]);
    frame();

    expect(writes).toHaveBeenCalledTimes(1);
    expect(writes.mock.calls[0][0]).toEqual(new Uint8Array([...wrote[0], ...wrote[1]]));
  });

  it("draws a character the program's output cut between two frames whole", async () => {
    await openPane();

    await send(new Uint8Array([0x63, 0x61, 0x66, 0xc3]));
    frame();
    await send(new Uint8Array([0xa9, 0x21]));
    frame();

    await waitFor(() =>
      expect(made[0].buffer.active.getLine(0)?.translateToString(true)).toBe("café!"),
    );
  });

  // xterm 6's UTF-8 decoder (`Utf8ToUtf32`) skips a byte that starts no character, and a
  // character whose continuation never comes; it draws no U+FFFD in their place, as the core's
  // `Utf8Stream` did.
  it("draws nothing for a byte that is no character", async () => {
    await openPane();

    await send(new Uint8Array([0x61, 0xff, 0x62, 0xc3, 0x63]));
    frame();

    await waitFor(() =>
      expect(made[0].buffer.active.getLine(0)?.translateToString(true)).toBe("abc"),
    );
  });

  // The core writes RESET (`ESC c`) before a view's next epoch. A character the old epoch cut
  // short must not be finished by the new screen's first bytes.
  it("does not finish a character cut short before a reset with the new screen's bytes", async () => {
    await openPane();

    await send(new Uint8Array([0x6f, 0x6c, 0x64, 0xc3]));
    frame();
    await send("\x1bc");
    await send(new Uint8Array([0xa9, 0x6e, 0x65, 0x77]));
    frame();

    await waitFor(() =>
      expect(made[0].buffer.active.getLine(0)?.translateToString(true)).toBe("new"),
    );
  });

  it("does not finish a character cut short before a reset gathered in the same frame", async () => {
    await openPane();

    await send(new Uint8Array([0x6f, 0x6c, 0x64, 0xe2, 0x94]));
    await send("\x1bc");
    await send(new Uint8Array([0x80, 0x6e, 0x65, 0x77]));
    frame();

    await waitFor(() =>
      expect(made[0].buffer.active.getLine(0)?.translateToString(true)).toBe("new"),
    );
  });
});

describe("what the terminal drew, told back to the core (#4733)", () => {
  /** Each `session_drawn` the pane sent, as it sent it. */
  let reports: unknown[] = [];
  beforeEach(() => {
    reports = [];
    mockIPC((cmd, args) => {
      if (cmd === "watch_session") {
        output = (args as { output: Channel<ArrayBuffer> }).output;
        return { view: 1, columns: 80, rows: 24, scrollback: 5000, newline: null };
      }
      if (cmd === "session_drawn") reports.push(args);
      return null;
    });
  });

  it("reports nothing while what it was sent is not drawn", async () => {
    await openPane();

    await send("first ");
    await send("second");

    expect(reports).toEqual([]);
  });

  it("reports once a write is parsed, counting each chunk the write carried", async () => {
    await openPane();

    await send("first ");
    await send("second ");
    await send("third");
    frame();

    await waitFor(() =>
      expect(reports).toEqual([{ plane: "/planes/one", session: 1, view: 1, pieces: 3 }]),
    );
    expect(made[0].buffer.active.getLine(0)?.translateToString(true)).toBe("first second third");
  });
});
