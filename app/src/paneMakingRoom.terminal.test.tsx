// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useRef, useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { FitAddon } from "@xterm/addon-fit";
import type { Terminal } from "@xterm/xterm";
import { SessionPane } from "./SessionPane";
import { letGoOfKeptTerminals } from "./keptTerminals";
import { dropPane, GRIP, usePaneDrag } from "./paneDrag";
import { PANES_ROOM } from "./paneHeld";
import type { DropContext } from "./paneDrop";
import { boxesOf } from "./panePreview";
import { contentsOf, insertPane, noTabs, openTab, type Tabs } from "./tabs";

/**
 * **A terminal is told its size once, on the drop** (#5291), with the real pane and the real
 * xterm. jsdom lays nothing out, so this does what a browser's layout does for it: each box is
 * what the window's layout gives it, or the pixels written on it, and a resize observer hears only
 * an element whose box changed ({@link layOut}). A fit reads the holder as `@xterm/addon-fit` reads
 * its parent, and the program hears what xterm says it resized to.
 */

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

const ctx: DropContext = {
  filedIn: () => "ops",
  name: (content) => (content.kind === "session" ? `chat ${content.session}` : "the empty pane"),
};
const ROOM = { left: 0, top: 0, width: 1600, height: 600 };

function start(): Tabs {
  let tabs = openTab(noTabs(), 1, "1", "steward");
  const tab = tabs.order[0];
  return insertPane(tabs, {
    tab,
    pane: contentsOf(tabs, tab)[0].pane,
    direction: "row",
    side: "after",
    content: { kind: "session", session: 3, chat: "3" },
  });
}

function Window({ first }: { first: Tabs }) {
  const [tabs, setTabs] = useState(first);
  const now = useRef(first);
  const change = useCallback((how: (tabs: Tabs) => Tabs) => {
    const next = how(now.current);
    now.current = next;
    setTabs(next);
    return next;
  }, []);
  const hold = useCallback(() => {}, []);
  const own = useCallback((session: number) => String(session), []);
  const drag = usePaneDrag({ tabs, change, ctx, onHoldTab: hold, chatNameOf: own });
  const front = tabs.inFront ?? tabs.order[0];
  const boxes = boxesOf(tabs.byId[front].layout, tabs.byId[front].focused, ROOM, {
    separator: 0,
    fold: 0,
  });
  return (
    <div {...{ [PANES_ROOM]: "" }} data-box="0,0,1600,600">
      {contentsOf(tabs, front).map(({ pane, content }) => {
        const box = boxes.panes.get(pane);
        const session = content.kind === "session" ? content.session : 0;
        return (
          // Keyed by the chat: a pane moved is its terminal, taken up and put back.
          <div
            key={session}
            {...dropPane(front, pane)}
            data-box={`${box?.left},${box?.top},${box?.width},${box?.height}`}
          >
            <div {...{ [GRIP]: "" }}>{`head ${session}`}</div>
            <SessionPane
              plane="/planes/one"
              session={session}
              focused={false}
              onFocus={() => {}}
            />
          </div>
        );
      })}
      {drag.layer}
    </div>
  );
}

const CELL = 10;

/** The box an element is drawn at: the pixels written on it, else its frame's, else its own. */
function boxOf(el: Element): { left: number; top: number; width: number; height: number } {
  const own = el.getAttribute("data-box");
  const written = el as HTMLElement;
  const pixels = (value: string) => (value === "" ? undefined : parseFloat(value));
  const from = own !== null
    ? (() => {
        const [left, top, width, height] = own.split(",").map(Number);
        return { left, top, width, height };
      })()
    : el.parentElement !== null
      ? boxOf(el.parentElement)
      : { left: 0, top: 0, width: 0, height: 0 };
  return {
    ...from,
    width: pixels(written.style?.width ?? "") ?? from.width,
    height: pixels(written.style?.height ?? "") ?? from.height,
  };
}

/** Every observer the panes made, and what each heard last. */
const watched: { owner: object; heard: () => void; on: Element; size: string }[] = [];
const sizeOf = (el: Element) => {
  const { width, height } = boxOf(el);
  return `${width}×${height}`;
};

/** What a browser does after a frame: an observer hears an element whose box changed. */
function layOut() {
  act(() => {
    for (const one of watched) {
      if (!one.on.isConnected || sizeOf(one.on) === one.size) continue;
      one.size = sizeOf(one.on);
      one.heard();
    }
  });
}

const told: [number, number][] = [];

beforeEach(() => {
  watched.length = 0;
  told.length = 0;
  vi.useFakeTimers();
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(private readonly heard: () => void) {}
      observe(on: Element) {
        // As a browser does, it is heard once when it starts being watched.
        watched.push({ owner: this, heard: () => this.heard(), on, size: "" });
      }
      unobserve() {}
      disconnect() {
        for (let at = watched.length - 1; at >= 0; at--)
          if (watched[at].owner === this) watched.splice(at, 1);
      }
    },
  );
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
    const { left: x, top: y, width, height } = boxOf(this);
    return DOMRect.fromRect({ x, y, width, height });
  });
  document.elementFromPoint = () => null;
  // As the addon reads its parent: whole cells of what the holder has.
  vi.spyOn(FitAddon.prototype, "fit").mockImplementation(function (this: FitAddon) {
    const terminal = (this as unknown as { _terminal: Terminal })._terminal;
    const holder = terminal.element?.parentElement;
    if (!holder) return;
    const { width, height } = boxOf(holder);
    const columns = Math.floor(width / CELL);
    const rows = Math.floor(height / CELL);
    if (columns > 0 && rows > 0 && (terminal.cols !== columns || terminal.rows !== rows))
      terminal.resize(columns, rows);
  });
  mockIPC((cmd, args) => {
    if (cmd === "resize_session") {
      const { columns, rows, cols } = args as { columns?: number; cols?: number; rows: number };
      told.push([columns ?? cols ?? 0, rows]);
      return null;
    }
    return new Promise(() => {});
  });
});

afterEach(() => {
  cleanup();
  clearMocks();
  letGoOfKeptTerminals();
  vi.restoreAllMocks();
  vi.useRealTimers();
});

const at = (x: number, y: number) => ({ clientX: x, clientY: y, button: 0, isPrimary: true });
const head = (session: number) => screen.getByText(`head ${session}`);

/** The window drawn, each terminal fitted to its pane as it first comes up, and that forgotten. */
function shown() {
  render(<Window first={start()} />);
  layOut();
  told.length = 0;
}

describe("a terminal while its pane is dragged (#5291)", () => {
  it("is told nothing during a drag, whatever its pane does, and its size once on the drop", () => {
    shown();
    fireEvent.pointerDown(head(3), at(900, 10));
    for (const [x, y] of [
      [60, 10],
      [60, 300],
      [400, 590],
      [100, 590],
      [60, 10],
    ]) {
      fireEvent.pointerMove(document, at(x, y));
      act(() => vi.advanceTimersByTime(70));
      layOut();
    }
    expect(told).toEqual([]);
    fireEvent.pointerUp(document, at(60, 10));
    layOut();
    // Over chat 1's top: each pane is the whole width at half the height, and each is told once.
    expect(told.slice().sort()).toEqual([
      [160, 30],
      [160, 30],
    ]);
    layOut();
    expect(told).toHaveLength(2);
  });

  it("is told nothing when the drag is cancelled", () => {
    shown();
    fireEvent.pointerDown(head(3), at(900, 10));
    fireEvent.pointerMove(document, at(60, 10));
    act(() => vi.advanceTimersByTime(70));
    layOut();
    fireEvent.keyDown(document, { key: "Escape" });
    layOut();
    expect(told).toEqual([]);
  });
});
