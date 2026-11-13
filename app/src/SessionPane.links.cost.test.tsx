// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Terminal } from "@xterm/xterm";
import { letGoOfKeptTerminals } from "./keptTerminals";
import { SessionPane } from "./SessionPane";

/**
 * **What a pane's links cost while its output streams** (#6215; spec #6208: "detection must not
 * slow a pane while output streams"), counted in buffer lines the link hook reads, as the other
 * `*.cost.test.tsx` count React's commits.
 *
 * xterm asks a pane's link provider only for the line under the pointer, and again as that line
 * changes. So the hook's work is per write and per line hovered, never per line written: ten
 * times the output costs it no more. **Held by counts, which are the same on every machine**
 * (ADR 0086): how many buffer lines the hook read.
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

const WIDTH = 8;
const CELL = 17;
const measuring = (el: HTMLElement) => el.classList.contains("xterm-char-measure-element");

/** The writes output streams in. */
const WRITES = 20;

/**
 * The most buffer lines the hook may read per write with the pointer resting on an unwrapped
 * line: the line, and the one before and after it, to see it wraps over neither.
 */
const PER_WRITE = 4;

beforeEach(() => {
  made.length = 0;
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockImplementation(function (
    this: HTMLElement,
  ) {
    return measuring(this) ? WIDTH * 32 : 0;
  });
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockImplementation(function (
    this: HTMLElement,
  ) {
    return measuring(this) ? CELL : 0;
  });
  mockIPC((cmd, args) => {
    if (cmd === "watch_session")
      return { view: 1, columns: 80, rows: 24, scrollback: 5000, newline: null };
    if (cmd === "pane_link_resolve") {
      const { candidates } = args as { candidates: { text: string }[] };
      return candidates.map((one) => ({
        kind: "url",
        target: one.text,
        shown: one.text,
        does: { does: "opens", into: "browser" },
      }));
    }
    return null;
  });
});
afterEach(() => {
  cleanup();
  letGoOfKeptTerminals();
  clearMocks();
  vi.restoreAllMocks();
});

const write = (term: Terminal, text: string) => new Promise<void>((done) => term.write(text, done));

/** Buffer lines read through xterm's API, which only the link hook reads while output streams. */
async function linesReadStreaming(lines: number): Promise<number> {
  render(<SessionPane plane="/planes/one" session={1} focused={false} onFocus={() => {}} />);
  const term = made[0];
  await waitFor(() => expect(term.cols).toBe(80));
  const drawn = term.element?.querySelector(".xterm-screen");
  if (!drawn) throw new Error("the pane opened no terminal screen");
  const view = Object.getPrototypeOf(term.buffer.active) as { getLine: () => unknown };
  const read = vi.spyOn(view, "getLine");
  // The pointer rests on the pane, on a line output keeps writing under.
  drawn.dispatchEvent(
    new MouseEvent("mousemove", {
      clientX: 20 * WIDTH + WIDTH / 2,
      clientY: 10 * CELL + CELL / 2,
      bubbles: true,
    }),
  );

  const per = lines / WRITES;
  for (let at = 0; at < WRITES; at += 1) {
    let chunk = "";
    for (let line = 0; line < per; line += 1)
      chunk += `line ${at * per + line}: https://purlis.dev/${at}/${line}\r\n`;
    await write(term, chunk);
  }
  await new Promise((settle) => setTimeout(settle, 50));
  const count = read.mock.calls.length;
  read.mockRestore();
  cleanup();
  letGoOfKeptTerminals();
  made.length = 0;
  return count;
}

describe("what a pane's links cost while output streams", () => {
  it("reads a few lines per write under a resting pointer, however much is written", async () => {
    const few = await linesReadStreaming(200);
    const many = await linesReadStreaming(2000);

    expect(many).toBeLessThanOrEqual((WRITES + 1) * PER_WRITE);
    // Ten times the output: no more read.
    expect(many).toBeLessThanOrEqual(few + PER_WRITE);
  });
});
