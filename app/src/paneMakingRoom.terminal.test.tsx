// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useRef, useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SessionPane } from "./SessionPane";
import { letGoOfKeptTerminals } from "./keptTerminals";
import { dropPane, GRIP, usePaneDrag } from "./paneDrag";
import { PANES_ROOM } from "./paneHeld";
import type { DropContext } from "./paneDrop";
import { boxesOf } from "./panePreview";
import { at, layOut, layOutAsABrowserDoes } from "./paneTerminals.testkit";
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
  const tabs = openTab(noTabs(), 1, "1", "steward");
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
  // What a hold brought forward was told as it came: there is nothing more to tell.
  const told = useCallback(() => undefined, []);
  const own = useCallback((session: number) => String(session), []);
  const drag = usePaneDrag({
    tabs,
    change,
    ctx,
    onHoldTab: hold,
    onHoldOver: told,
    chatNameOf: own,
  });
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
            <SessionPane plane="/planes/one" session={session} focused={false} onFocus={() => {}} />
          </div>
        );
      })}
      {drag.layer}
    </div>
  );
}

const told: [number, number][] = [];

beforeEach(() => {
  told.length = 0;
  vi.useFakeTimers();
  layOutAsABrowserDoes();
  document.elementFromPoint = () => null;
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
