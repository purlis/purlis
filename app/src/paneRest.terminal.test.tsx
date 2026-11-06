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
import { forgetKeyboard, giveKeyboardTo } from "./paneKeyboard";
import { dropPane, GRIP, HOLD_MS, STRIP, TAB, usePaneDrag } from "./paneDrag";
import { PANES_ROOM } from "./paneHeld";
import type { DropContext } from "./paneDrop";
import { boxesOf } from "./panePreview";
import { contentsOf, insertPane, noTabs, openTab, selectTab, type Tabs } from "./tabs";

/**
 * **Resting on a tab during a drag** (#5523), with the real pane and the real xterm: a tab brought
 * forward mounts its panes and their terminals before the drop. The window here holds a tab
 * forward as `PlaneView` does (#5294): drawn in front, told to nobody until the drag is over.
 * jsdom lays nothing out, so each box is what the layout gives it or the pixels a drag writes on
 * it, and a resize observer hears only an element whose box changed ({@link layOut}).
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
const ROOM = { left: 0, top: 40, width: 1600, height: 600 };

/** One tab: chat 1 left, chat 3 right; then a second tab, chat 4. The first is in front. */
function start(): Tabs {
  let tabs = openTab(noTabs(), 1, "1", "steward");
  const tab = tabs.order[0];
  tabs = insertPane(tabs, {
    tab,
    pane: contentsOf(tabs, tab)[0].pane,
    direction: "row",
    side: "after",
    content: { kind: "session", session: 3, chat: "3" },
  });
  tabs = openTab(tabs, 4, "4", "steward");
  return selectTab(tabs, tab);
}

function Window({ first, room }: { first: Tabs; room: () => typeof ROOM }) {
  const [tabs, setTabs] = useState(first);
  const now = useRef(first);
  const change = useCallback((how: (tabs: Tabs) => Tabs) => {
    const next = how(now.current);
    now.current = next;
    setTabs(next);
    return next;
  }, []);
  // As `PlaneView`'s `holdTab`: drawn in front, and no pane noted as used.
  const hold = useCallback((id: number) => {
    now.current = selectTab(now.current, id);
    setTabs(now.current);
  }, []);
  const holdOver = useCallback(() => undefined, []);
  const own = useCallback((session: number) => String(session), []);
  const drag = usePaneDrag({
    tabs,
    change,
    ctx,
    onHoldTab: hold,
    onHoldOver: holdOver,
    chatNameOf: own,
  });
  const front = tabs.inFront ?? tabs.order[0];
  const { left, top, width, height } = room();
  const boxes = boxesOf(tabs.byId[front].layout, tabs.byId[front].focused, room(), {
    separator: 0,
    fold: 0,
  });
  return (
    <div>
      <div {...{ [STRIP]: "" }} data-box="0,0,1600,30">
        {tabs.order.map((id, at) => (
          <div key={id} {...{ [TAB]: String(id) }} data-box={`${at * 100},0,100,30`}>
            <span role="tab" aria-selected={id === front}>{`tab ${id}`}</span>
          </div>
        ))}
      </div>
      <div {...{ [PANES_ROOM]: "" }} data-box={`${left},${top},${width},${height}`}>
        {contentsOf(tabs, front).map(({ pane, content }) => {
          const box = boxes.panes.get(pane);
          const session = content.kind === "session" ? content.session : 0;
          return (
            <div
              key={session}
              {...dropPane(front, pane)}
              data-box={`${box?.left},${box?.top},${box?.width},${box?.height}`}
            >
              <div {...{ [GRIP]: "" }}>{`head ${session}`}</div>
              <SessionPane
                plane="/planes/one"
                session={session}
                focused={pane === tabs.byId[front].focused}
                onFocus={() => {}}
              />
            </div>
          );
        })}
      </div>
      <input aria-label="elsewhere" data-box="0,800,200,20" />
      {drag.layer}
    </div>
  );
}

const CELL = 10;

type Box = { left: number; top: number; width: number; height: number };

/** The box an element is drawn at: the pixels written on it, else its frame's, else its own. */
function boxOf(el: Element): Box {
  const own = el.getAttribute("data-box");
  const written = el as HTMLElement;
  const pixels = (value: string) => (value === "" ? undefined : parseFloat(value));
  const from =
    own !== null
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

/** What the core was asked, in order, from when the window was drawn and fitted. */
const asked: { cmd: string; args: Record<string, unknown> }[] = [];
const resized = () =>
  asked
    .filter(({ cmd }) => cmd === "resize_session")
    .map(({ args }) => [args.session, args.columns ?? args.cols, args.rows]);
/** Everything the core was asked that is about the keyboard: what is typed, or a pane used. */
const aboutTheKeyboard = () =>
  asked.filter(({ cmd }) => ["send_input", "send_input_bytes", "pane_used"].includes(cmd));

beforeEach(() => {
  watched.length = 0;
  asked.length = 0;
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
  // The strip is found by the point; the panes never are: their zones stay put.
  document.elementFromPoint = (x: number, y: number) => {
    const holding = [
      ...document.querySelectorAll(`[${STRIP}], [${TAB}]:not([data-drop-pane])`),
    ].filter((el) => {
      const b = boxOf(el);
      return x >= b.left && x < b.left + b.width && y >= b.top && y < b.top + b.height;
    });
    return holding[holding.length - 1] ?? null;
  };
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
    asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
    if (cmd === "resize_session") return null;
    return new Promise(() => {});
  });
});

afterEach(() => {
  cleanup();
  clearMocks();
  letGoOfKeptTerminals();
  forgetKeyboard();
  vi.restoreAllMocks();
  vi.useRealTimers();
});

const at = (x: number, y: number) => ({ clientX: x, clientY: y, button: 0, isPrimary: true });
const head = (session: number) => screen.getByText(`head ${session}`);
/** The chat whose terminal has the keyboard, if any does. */
const typingInto = () => {
  const pane = document.activeElement?.closest("[data-drop-pane]");
  return pane?.querySelector("[data-pane-terminal]") == null
    ? undefined
    : pane.textContent?.match(/head (\d+)/)?.[1];
};

/**
 * The window drawn, with the second tab drawn once and put behind, so its terminal is kept as a
 * tab behind keeps it; each terminal fitted to its pane, and that forgotten. The keyboard is in
 * the field outside the panes.
 */
function shown(room: { now: typeof ROOM } = { now: ROOM }) {
  const first = start();
  render(<Window first={selectTab(first, first.order[1])} room={() => room.now} />);
  layOut();
  cleanup();
  render(<Window first={first} room={() => room.now} />);
  layOut();
  screen.getByLabelText("elsewhere").focus();
  asked.length = 0;
  return room;
}

/** Chat 3 picked up by its head and rested on the second tab until it comes forward. */
function restOnTheSecondTab() {
  fireEvent.pointerDown(head(3), at(900, 50));
  fireEvent.pointerMove(document, at(150, 10));
  act(() => vi.advanceTimersByTime(HOLD_MS));
  layOut();
  expect(screen.getByText("head 4")).toBeTruthy();
}

describe("resting on a tab during a drag (#5523)", () => {
  it("resizes no terminal of either tab when the rest is cancelled", () => {
    shown();
    restOnTheSecondTab();
    // Over chat 4's pane, which makes room for chat 3 at each of its edges in turn.
    for (const [x, y] of [
      [1590, 340],
      [800, 630],
      [10, 340],
      [800, 50],
    ]) {
      fireEvent.pointerMove(document, at(x, y));
      act(() => vi.advanceTimersByTime(70));
      layOut();
    }
    fireEvent.keyDown(document, { key: "Escape" });
    act(() => vi.advanceTimersByTime(1));
    layOut();
    expect(screen.getByText("head 1")).toBeTruthy();
    expect(resized()).toEqual([]);
  });

  it("keeps the keyboard where it was while a tab is held forward, though its chat is owed it", () => {
    shown();
    restOnTheSecondTab();
    // Something asks for chat 4's terminal to have the keyboard while the drag holds its tab.
    act(() => giveKeyboardTo("/planes/one", 4));
    expect(typingInto()).toBeUndefined();
    expect(document.activeElement).toBe(screen.getByLabelText("elsewhere"));
    fireEvent.keyDown(document, { key: "Escape" });
    act(() => vi.advanceTimersByTime(1));
    expect(document.activeElement).toBe(screen.getByLabelText("elsewhere"));
    expect(aboutTheKeyboard()).toEqual([]);
  });

  it("owes nothing once the drag is over: what it held back is dropped, on a cancel or a drop", () => {
    shown();
    restOnTheSecondTab();
    act(() => giveKeyboardTo("/planes/one", 4));
    fireEvent.keyDown(document, { key: "Escape" });
    act(() => vi.advanceTimersByTime(1));
    restOnTheSecondTab();
    act(() => giveKeyboardTo("/planes/one", 4));
    fireEvent.pointerMove(document, at(1590, 340));
    act(() => vi.advanceTimersByTime(70));
    fireEvent.pointerUp(document, at(1590, 340));
    act(() => vi.advanceTimersByTime(1));
    layOut();
    expect(screen.getByText("head 4")).toBeTruthy();
    expect(typingInto()).toBeUndefined();
    expect(aboutTheKeyboard()).toEqual([]);
  });

  it("fits a terminal brought forward to a window resized while its tab was behind, and nothing back on a cancel", () => {
    const room = shown();
    room.now = { ...ROOM, width: 1200 };
    restOnTheSecondTab();
    // Only chat 4's terminal, which is drawn at the window's size now: it would be on its next show.
    expect(resized()).toEqual([[4, 120, 60]]);
    fireEvent.keyDown(document, { key: "Escape" });
    act(() => vi.advanceTimersByTime(1));
    layOut();
    expect(resized().filter(([session]) => session === 4)).toHaveLength(1);
  });
});
