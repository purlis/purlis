// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Channel } from "@tauri-apps/api/core";
import type { Terminal } from "@xterm/xterm";
import { SessionPane } from "./SessionPane";
import { chatShown, keptKey, letGoOfKeptTerminals } from "./keptTerminals";
import { forgetStuckWrites, UNPARSED_MS } from "./stuckWrites";

/**
 * **A pane whose terminal stops parsing what it is written starts again** (#5478,
 * `stuckWrites.ts`). xterm.js 6.0.0 threw inside its write loop, which then never ran again: the
 * pane stopped reporting what it drew, the host stopped sending at its watermark, and the pane
 * stayed frozen half-drawn until it fell out of the kept chats. Now the pane does what falling
 * out of them does, as soon as it sees it.
 *
 * The real SessionPane and the real xterm, as in `SessionPane.noEcho.test.tsx`; frames and the
 * watch's clock are the test's (`setInterval` and `Date` faked, so xterm's own timers still run).
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

vi.mock("./renderer", async (real) => ({
  ...(await real<typeof import("./renderer")>()),
  draw: () => new Promise(() => {}),
}));

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
/** The next frame: what arrived since the last one is written to the terminal. */
const frame = () =>
  act(() => {
    const due = queued;
    queued = new Map();
    for (const then of due.values()) then(performance.now());
  });

let outputs: Channel<ArrayBuffer>[] = [];
let calls: { cmd: string; args: Record<string, unknown> }[] = [];
let views = 0;
let index = 0;

const send = (chunk: string) =>
  act(() => {
    const output = outputs[outputs.length - 1];
    if (!output) throw new Error("the pane has opened no view");
    const bytes = new TextEncoder().encode(chunk);
    window.__TAURI_INTERNALS__.runCallback(output.id, { index: index++, message: bytes.buffer });
  });
const later = (ms: number) =>
  act(async () => {
    vi.advanceTimersByTime(ms);
    await Promise.resolve();
  });
/** xterm's own write loop runs on real timers: what it was written is parsed, and reported. */
const parsed = () => act(() => new Promise((then) => setTimeout(then, 20)));
const named = (cmd: string) => calls.filter((call) => call.cmd === cmd);

let visibility: DocumentVisibilityState = "visible";

beforeEach(() => {
  made.length = 0;
  outputs = [];
  calls = [];
  views = 0;
  index = 0;
  queued = new Map();
  visibility = "visible";
  vi.spyOn(document, "visibilityState", "get").mockImplementation(() => visibility);
  vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "Date"] });
  vi.stubGlobal("requestAnimationFrame", (then: FrameRequestCallback) => {
    nextId += 1;
    queued.set(nextId, then);
    return nextId;
  });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => queued.delete(id));
  mockIPC((cmd, args) => {
    calls.push({ cmd, args: args as Record<string, unknown> });
    if (cmd === "watch_session") {
      outputs.push((args as { output: Channel<ArrayBuffer> }).output);
      views += 1;
      // Each view's channel counts its messages from the first.
      index = 0;
      return { view: views, columns: 80, rows: 24, scrollback: 5000, newline: null };
    }
    return null;
  });
});
afterEach(() => {
  cleanup();
  letGoOfKeptTerminals();
  forgetStuckWrites();
  clearMocks();
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.stubGlobal("requestAnimationFrame", ownFrame);
  vi.stubGlobal("cancelAnimationFrame", ownCancel);
});

const PANE = <SessionPane plane="/planes/one" session={7} focused={false} onFocus={() => {}} />;

async function openPane() {
  // One of the last chats shown, so its terminal would be kept: the case the freeze was seen in.
  chatShown(keptKey("/planes/one", 7));
  const view = render(PANE);
  await waitFor(() => expect(made[0]?.options.scrollback).toBe(5000));
  return { pane: made[0], view };
}

/** xterm's write loop has stopped: what the pane writes is never parsed, as after its throw. */
const stopParsing = (pane: Terminal) => vi.spyOn(pane, "write").mockImplementation(() => {});

describe("a pane whose terminal stops parsing what it is written", () => {
  it("starts its terminal and view again when a write throws", async () => {
    const { pane } = await openPane();
    vi.spyOn(pane, "write").mockImplementation(() => {
      throw new TypeError("undefined is not an object (evaluating 'd.setCellFromCodepoint')");
    });

    await send("the foot of the screen");
    frame();

    await waitFor(() => expect(named("watch_session")).toHaveLength(2));
    expect(named("unwatch_session")).toEqual([
      { cmd: "unwatch_session", args: expect.objectContaining({ session: 7, view: 1 }) },
    ]);
    await waitFor(() => expect(made).toHaveLength(2));
    expect(made[1]).not.toBe(pane);
  });

  it("starts them again when what it was written waits to be parsed", async () => {
    const { pane } = await openPane();
    stopParsing(pane);

    await send("never parsed");
    frame();
    await later(UNPARSED_MS - 1_000);
    expect(named("watch_session")).toHaveLength(1);
    await later(2_000);

    await waitFor(() => expect(named("watch_session")).toHaveLength(2));
    expect(named("unwatch_session")).toEqual([
      { cmd: "unwatch_session", args: expect.objectContaining({ view: 1 }) },
    ]);
    // The new terminal parses, and reports what it drew to its new view.
    await waitFor(() => expect(made[1]?.options.scrollback).toBe(5000));
    await send("drawn again");
    frame();
    await parsed();
    expect(named("session_drawn")).toContainEqual({
      cmd: "session_drawn",
      args: expect.objectContaining({ view: 2 }),
    });
  });

  it("keeps its terminal while what it is written is parsed", async () => {
    await openPane();

    for (let turn = 0; turn < 10; turn++) {
      await send(`line ${turn}\r\n`);
      frame();
      await parsed();
      expect(named("session_drawn")).toHaveLength(turn + 1);
      await later(3_000);
    }

    expect(named("watch_session")).toHaveLength(1);
    expect(named("unwatch_session")).toEqual([]);
  });

  it("waits for a hidden window to be shown before it says a write is stuck", async () => {
    const { pane } = await openPane();
    stopParsing(pane);

    visibility = "hidden";
    await send("written while hidden");
    frame();
    await later(UNPARSED_MS * 3);
    expect(named("watch_session")).toHaveLength(1);

    visibility = "visible";
    await later(UNPARSED_MS - 2_000);
    expect(named("watch_session")).toHaveLength(1);
    await later(3_000);
    await waitFor(() => expect(named("watch_session")).toHaveLength(2));
  });

  it("lets go of a kept terminal that stops, so its chat starts again when shown", async () => {
    const { pane, view } = await openPane();
    stopParsing(pane);
    // The person shows another chat: this one's terminal is kept, off the window.
    view.unmount();
    expect(named("unwatch_session")).toEqual([]);

    await send("taken while kept");
    frame();
    await later(UNPARSED_MS + 1_000);

    expect(named("unwatch_session")).toEqual([
      { cmd: "unwatch_session", args: expect.objectContaining({ view: 1 }) },
    ]);
    render(PANE);
    await waitFor(() => expect(made).toHaveLength(2));
    await waitFor(() => expect(named("watch_session")).toHaveLength(2));
  });
});
