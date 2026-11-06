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
 * [DEBUG-5478] **The freeze watchdog's line** (#5478): a pane whose terminal draws nothing for
 * seconds after output arrives writes one line to the diagnostic log, and a pane that draws
 * writes none.
 *
 * The real SessionPane and the real xterm, as in `SessionPane.output.test.tsx`; frames are the
 * test's, and so is the watchdog's clock (`setInterval` and `Date` faked, nothing else, so
 * xterm's own parse timers still run). A freeze is forced the way the #5478 loop's
 * `FORCE_FREEZE` forces one: the terminal's `write` does nothing, so nothing is ever drawn.
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
const frame = () =>
  act(() => {
    const due = queued;
    queued = new Map();
    for (const then of due.values()) then(performance.now());
  });

let output: Channel<ArrayBuffer> | undefined;
let index = 0;
const send = (chunk: string) =>
  act(() => {
    if (!output) throw new Error("the pane has opened no view");
    const bytes = new TextEncoder().encode(chunk);
    window.__TAURI_INTERNALS__.runCallback(output.id, { index: index++, message: bytes.buffer });
  });
/** The watchdog's clock moves on by `ms`. */
const later = (ms: number) =>
  act(async () => {
    vi.advanceTimersByTime(ms);
    // The line goes to the core through the IPC, which answers a microtask later.
    await Promise.resolve();
  });

/** The lines the window sent the diagnostic log. */
let logged: string[] = [];

beforeEach(() => {
  made.length = 0;
  output = undefined;
  index = 0;
  logged = [];
  queued = new Map();
  vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "Date"] });
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
    if (cmd === "debug_5478_log") logged.push((args as { line: string }).line);
    return null;
  });
});
afterEach(() => {
  cleanup();
  letGoOfKeptTerminals();
  clearMocks();
  vi.useRealTimers();
  vi.stubGlobal("requestAnimationFrame", ownFrame);
  vi.stubGlobal("cancelAnimationFrame", ownCancel);
});

async function openPane() {
  render(<SessionPane plane="/planes/one" session={7} focused={false} onFocus={() => {}} />);
  await waitFor(() => expect(made[0]?.options.scrollback).toBe(5000));
  // What opening asked xterm to draw is drawn, so a draw later is one of what came after.
  frame();
  frame();
  return made[0];
}

/** Output arrives, is written, parsed and drawn: what a pane that works does with it. */
async function drawn(pane: Terminal, text: string) {
  const rendered = vi.fn();
  const listening = pane.onRender(rendered);
  await send(text);
  frame();
  // xterm parses on a timer of its own, then asks for a frame to draw what it parsed.
  await waitFor(() => expect(pane.buffer.active.getLine(0)?.translateToString(true)).not.toBe(""));
  await waitFor(() => expect(queued.size).toBeGreaterThan(0));
  frame();
  listening.dispose();
  expect(rendered).toHaveBeenCalled();
}

describe("[DEBUG-5478] the freeze watchdog", () => {
  it("writes nothing while output arrives and the terminal draws it", async () => {
    const pane = await openPane();

    for (let turn = 0; turn < 5; turn++) {
      await drawn(pane, `line ${turn}\r\n`);
      await later(2_000);
    }
    await later(10_000);

    expect(logged).toEqual([]);
  });

  it("writes one line with xterm's state when output arrives and nothing is drawn", async () => {
    const pane = await openPane();
    vi.spyOn(pane, "write").mockImplementation(() => {});

    await send("never drawn");
    frame();
    await later(2_000);
    expect(logged).toEqual([]);
    await later(2_000);

    expect(logged).toHaveLength(1);
    expect(logged[0]).toMatch(/^pane froze: \{/);
    const said = JSON.parse(logged[0].slice("pane froze: ".length)) as Record<string, unknown>;
    expect(said).toMatchObject({
      session: 7,
      by: "output",
      outputs: 1,
      writes: 1,
      isPaused: false,
      synchronizedOutputMode: false,
      writeBuffer: { length: expect.any(Number), offset: expect.any(Number) },
      errors: [],
      pane: { cols: expect.any(Number), rows: expect.any(Number) },
      visibilityState: "visible",
    });
    expect(said).toHaveProperty("lastResizeSent");
    expect(said).toHaveProperty("intersecting");
  });

  it("writes at most one line a minute for a pane that stays frozen", async () => {
    const pane = await openPane();
    vi.spyOn(pane, "write").mockImplementation(() => {});

    await send("never drawn");
    frame();
    await later(4_000);
    for (let second = 0; second < 50; second += 5) {
      await send("still nothing");
      frame();
      await later(5_000);
    }
    expect(logged).toHaveLength(1);

    await later(10_000);
    await send("a minute on");
    frame();
    await later(4_000);
    expect(logged).toHaveLength(2);
  });

  it("writes its line while a write waits unparsed, though the cursor's row is redrawn", async () => {
    const pane = await openPane();
    vi.spyOn(pane, "write").mockImplementation(() => {});
    const redrawn = vi.fn();
    pane.onRender(redrawn);

    await send("never parsed");
    frame();
    for (let blink = 0; blink < 8; blink++) {
      // What a cursor's blink or a change of focus asks xterm for: one row drawn again.
      pane.refresh(pane.buffer.active.cursorY, pane.buffer.active.cursorY);
      frame();
      await later(500);
    }

    expect(redrawn).toHaveBeenCalled();
    expect(logged).toHaveLength(1);
    expect(logged[0]).toContain('"unparsed":1');
  });

  it("writes nothing for a kept terminal that no holder shows", async () => {
    const pane = await openPane();
    vi.spyOn(pane, "write").mockImplementation(() => {});
    // What keeping a terminal does as its chat leaves the pane (`SessionPane.tsx`).
    pane.element?.remove();

    await send("taken while kept");
    frame();
    await later(10_000);

    expect(logged).toEqual([]);
  });

  it("notes an exception the write path threw, and throws it on", async () => {
    const pane = await openPane();
    const thrown = new Error("the parser broke");
    vi.spyOn(pane, "write").mockImplementation(() => {
      throw thrown;
    });
    const uncaught: unknown[] = [];

    await send("breaks it");
    try {
      frame();
    } catch (err) {
      uncaught.push(err);
    }
    await later(4_000);

    expect(uncaught).toEqual([thrown]);
    expect(logged).toHaveLength(1);
    expect(logged[0]).toContain('"errors":["write: Error: the parser broke"]');
  });
});
