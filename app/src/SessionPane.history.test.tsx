// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Channel } from "@tauri-apps/api/core";
import type { Terminal } from "@xterm/xterm";
import { SessionPane } from "./SessionPane";
import { FIRST_PAGE, PAGE } from "./historyPages";

/**
 * **A view opens on the screen and a page of history, and older lines are paged in as the
 * person scrolls up** (#4742). The real xterm, wrapped only so the test can reach the instance
 * the pane made: what is asserted is what its buffer holds and which lines are in view.
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

const PLANE = "/planes/one";
const ROWS = 24;

/** Lines `from` to `to`, one a row, as a snapshot draws them. */
const lines = (from: number, to: number) =>
  Array.from({ length: to - from + 1 }, (_, n) => `line ${from + n}`).join("\r\n");

/** What the host keeps of the session: lines 1 to 524, the last 24 of them its screen. */
const LAST = 524;

/** The views the pane opened, in order: what each asked for, and the channel it is drawn on. */
let opened: { history: number | null; output: Channel<string> }[] = [];
let unwatched: number[] = [];

/** A host that keeps `LAST` lines and draws a view with the newest `history` of its history,
 *  or every line where it pages none, as a host before 1.69 does. `again`, when given, is what
 *  it does before answering a view after the first: refuse it, or wait to be let answer. */
function aHost(pages: boolean, again?: () => Promise<void>) {
  opened = [];
  unwatched = [];
  mockIPC(async (cmd, args) => {
    if (cmd === "watch_session") {
      const { history, output } = args as { history: number | null; output: Channel<string> };
      opened.push({ history, output });
      if (opened.length > 1 && again) await again();
      const kept = LAST - ROWS;
      const older = pages && history !== null ? Math.max(0, kept - history) : 0;
      return {
        view: opened.length,
        columns: 80,
        rows: ROWS,
        scrollback: 5000,
        newline: null,
        older,
      };
    }
    if (cmd === "unwatch_session") unwatched.push((args as { view: number }).view);
    return null;
  });
}

/** Draws view `n` (counted from 1) as the host would: its snapshot, from the first line it
 *  keeps. */
function draws(n: number, from: number) {
  act(() => opened[n - 1].output.onmessage(lines(from, LAST)));
}

const terminal = () => made[0];
const lineInView = () => {
  const buffer = terminal().buffer.active;
  return buffer.getLine(buffer.viewportY)?.translateToString(true);
};

/** xterm's own measuring span is given a cell's size, 8 by 17 CSS pixels: jsdom lays nothing
 *  out, and a terminal with no cell size scrolls nowhere. */
const measuring = (el: HTMLElement) => el.classList.contains("xterm-char-measure-element");

beforeEach(() => {
  made.length = 0;
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockImplementation(function (
    this: HTMLElement,
  ) {
    return measuring(this) ? 8 * 32 : 0;
  });
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockImplementation(function (
    this: HTMLElement,
  ) {
    return measuring(this) ? 17 : 0;
  });
});
afterEach(() => {
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const pane = () =>
  render(<SessionPane plane={PLANE} session={1} focused={false} onFocus={() => {}} />);

describe("a chat's terminal and its history (#4742)", () => {
  it("opens its view on the screen and the newest page of history", async () => {
    aHost(true);
    pane();

    await waitFor(() => expect(opened).toHaveLength(1));
    expect(opened[0].history).toBe(FIRST_PAGE);
  });

  it("pages older lines in as the person scrolls to the top, keeping the same line in view", async () => {
    aHost(true);
    pane();
    await waitFor(() => expect(opened).toHaveLength(1));
    const first = LAST - ROWS - FIRST_PAGE + 1;
    draws(1, first);
    await waitFor(() => expect(terminal().buffer.active.length).toBe(FIRST_PAGE + ROWS));

    act(() => terminal().scrollToTop());

    await waitFor(() => expect(opened).toHaveLength(2));
    expect(opened[1].history).toBe(FIRST_PAGE + PAGE);
    draws(2, 1);
    await waitFor(() => expect(terminal().buffer.active.length).toBe(LAST));
    await waitFor(() => expect(lineInView()).toBe(`line ${first}`));
    expect(terminal().buffer.active.getLine(0)?.translateToString(true)).toBe("line 1");
    expect(unwatched).toEqual([1]);
  });

  it("draws nothing more from the view it let go of", async () => {
    aHost(true);
    pane();
    await waitFor(() => expect(opened).toHaveLength(1));
    draws(1, LAST - ROWS - FIRST_PAGE + 1);
    await waitFor(() => expect(terminal().buffer.active.length).toBe(FIRST_PAGE + ROWS));
    act(() => terminal().scrollToTop());
    await waitFor(() => expect(opened).toHaveLength(2));
    draws(2, 1);
    await waitFor(() => expect(terminal().buffer.active.length).toBe(LAST));

    act(() => opened[0].output.onmessage("\r\nlate from the old view"));
    act(() => opened[1].output.onmessage("\r\nfrom the new one"));

    await waitFor(() => expect(terminal().buffer.active.length).toBe(LAST + 1));
    const last = terminal().buffer.active.getLine(LAST)?.translateToString(true);
    expect(last).toBe("from the new one");
  });

  /** The pane's view drawn, then scrolled to the top, so it asks for a second view. */
  async function scrolledToTheTop() {
    pane();
    await waitFor(() => expect(opened).toHaveLength(1));
    draws(1, LAST - ROWS - FIRST_PAGE + 1);
    await waitFor(() => expect(terminal().buffer.active.length).toBe(FIRST_PAGE + ROWS));
    act(() => terminal().scrollToTop());
    await waitFor(() => expect(opened).toHaveLength(2));
  }
  const lastLine = () => {
    const buffer = terminal().buffer.active;
    return buffer.getLine(buffer.length - 1)?.translateToString(true);
  };

  it("keeps drawing the view it has when the page is refused", async () => {
    aHost(true, () => Promise.reject(new Error("the host is busy")));
    await scrolledToTheTop();
    // Long enough for the refusal to have been taken in.
    await new Promise((settled) => setTimeout(settled, 50));

    act(() => opened[0].output.onmessage("\r\nstill streaming"));

    await waitFor(() => expect(lastLine()).toBe("still streaming"));
    expect(unwatched).toEqual([]);
  });

  it("keeps drawing the view it has until the new one answers", async () => {
    let answer = () => {};
    aHost(true, () => new Promise((answered) => (answer = answered)));
    await scrolledToTheTop();

    act(() => opened[0].output.onmessage("\r\nwhile the page is asked for"));
    await waitFor(() => expect(lastLine()).toBe("while the page is asked for"));

    act(() => answer());
    await waitFor(() => expect(unwatched).toEqual([1]));
    draws(2, 1);
    await waitFor(() => expect(terminal().buffer.active.length).toBe(LAST));
  });

  it("asks for nothing more of a host that drew all of it", async () => {
    aHost(false);
    pane();
    await waitFor(() => expect(opened).toHaveLength(1));
    draws(1, 1);
    await waitFor(() => expect(terminal().buffer.active.length).toBe(LAST));

    act(() => terminal().scrollToTop());

    // Long enough for a page to have been asked for, had it been.
    await new Promise((settled) => setTimeout(settled, 50));
    expect(opened).toHaveLength(1);
  });
});
