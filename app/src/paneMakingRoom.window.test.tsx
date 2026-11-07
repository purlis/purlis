// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CHAT_ROW, dropPane, GRIP, HOLD_MS, STRIP, TAB, usePaneDrag } from "./paneDrag";
import { keystroke, shortcut } from "./keys/registry";
import { onAMac } from "./tabKeys";
import { PANES_ROOM, RAIL, TERMINAL_HOLDER, terminalInPane } from "./paneHeld";
import type { DropContext } from "./paneDrop";
import { boxesOf } from "./panePreview";
import {
  closePane,
  contentsOf,
  focusPane,
  insertPane,
  noTabs,
  openTab,
  selectTab,
  type Tabs,
} from "./tabs";

/**
 * **Panes make room while dragging** (#5291, spec #5288), read as the person sees it: where each
 * pane is drawn, what is written by the pointer, and that the tabs have not changed until the
 * drop. jsdom lays nothing out, so this window lays its panes out by the same `boxesOf` and each
 * element says its box (`data-box`); a transform and a size the drag writes are read off the frame.
 * A real `DOMRect` keeps its sides as getters (#5284), so every measure is one.
 */

const ctx: DropContext = {
  filedIn: () => "ops",
  name: (content) => (content.kind === "session" ? `chat ${content.session}` : "the empty pane"),
};

const ROOM = { left: 0, top: 40, width: 1600, height: 600 };
const GAPS = { separator: 0, fold: 0 };
/** Each terminal: 10 × 20 cells, and 60 of its pane's height is the head and the foot. */
const CELLS = { width: 10, height: 20, extraWidth: 0, extraHeight: 0 };
const CHROME = 60;

/** One tab: chat 1 left, chat 3 right; then a second tab, chat 4. */
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

function Terminal() {
  // In a passive effect, as `SessionPane` does: after a tab brought forward has been measured.
  const at = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (at.current !== null) return terminalInPane(at.current, () => CELLS);
  }, []);
  return <div ref={at} {...{ [TERMINAL_HOLDER]: "" }} data-testid="holder" />;
}

function Window({
  first,
  heard,
  room = ROOM,
  active,
  control,
}: {
  first: Tabs;
  heard: (tabs: Tabs) => void;
  room?: typeof ROOM;
  active?: boolean;
  control?: (change: (how: (tabs: Tabs) => Tabs) => Tabs) => void;
}) {
  const [tabs, setTabs] = useState(first);
  const now = useRef(first);
  /** What the window last told: what `heard` was given. */
  const told = useRef(first);
  const change = useCallback(
    (how: (tabs: Tabs) => Tabs) => {
      const next = how(now.current);
      now.current = next;
      told.current = next;
      setTabs(next);
      heard(next);
      return next;
    },
    [heard],
  );
  useEffect(() => {
    control?.(change);
  }, [change, control]);
  // As the window does (#5294): a tab a drag rests on is drawn in front and told to nobody until
  // the drag is over, when what is drawn is told if it differs from what was.
  const hold = useCallback((id: number) => {
    now.current = selectTab(now.current, id);
    setTabs(now.current);
  }, []);
  const holdOver = useCallback(() => {
    if (JSON.stringify(now.current) === JSON.stringify(told.current)) return;
    told.current = now.current;
    heard(now.current);
  }, [heard]);
  const own = useCallback((session: number) => String(session), []);
  const drag = usePaneDrag({
    tabs,
    change,
    ctx,
    onHoldTab: hold,
    onHoldOver: holdOver,
    chatNameOf: own,
    active,
  });
  const front = tabs.inFront ?? tabs.order[0];
  const boxes = boxesOf(tabs.byId[front].layout, tabs.byId[front].focused, room, GAPS);
  return (
    <div>
      <div {...{ [STRIP]: "" }} data-box="0,0,1600,30">
        {tabs.order.map((id, at) => (
          <div key={id} {...{ [TAB]: String(id) }} data-box={`${at * 100},0,100,30`}>
            <span role="tab" aria-selected={id === tabs.inFront}>{`tab ${id}`}</span>
          </div>
        ))}
      </div>
      <div
        {...{ [PANES_ROOM]: "" }}
        data-box={`${room.left},${room.top},${room.width},${room.height}`}
      >
        {contentsOf(tabs, front).map(({ pane, content }) => {
          const box = boxes.panes.get(pane);
          if (box === undefined) return null;
          const { left, top, width, height } = box;
          return (
            <div
              key={pane}
              {...dropPane(front, pane)}
              data-testid={`frame ${content.kind === "session" ? content.session : "empty"}`}
              data-box={`${left},${top},${width},${height}`}
            >
              <div {...{ [GRIP]: "" }} className="pane-top" data-box={`${left},${top},${width},20`}>
                {`head ${content.kind === "session" ? content.session : "empty"}`}
              </div>
              <div
                className="pane-body"
                data-box={`${left},${top + CHROME},${width},${height - CHROME}`}
              >
                <Terminal />
              </div>
              <button type="button">{`Close ${content.kind === "session" ? content.session : "empty"}`}</button>
            </div>
          );
        })}
      </div>
      <button type="button" {...{ [CHAT_ROW]: "7" }} data-box="0,700,200,20">
        chat 7
      </button>
      <input aria-label="elsewhere" data-box="0,800,200,20" />
      {/* A side folded to a rail, as `PaneRail` draws it: no pane a drop can land in. */}
      <div {...{ [RAIL]: "" }} data-box="0,900,40,200">
        <button type="button">rail</button>
      </div>
      {drag.layer}
    </div>
  );
}

const boxOf = (el: Element) => {
  const [left, top, width, height] = (el.getAttribute("data-box") ?? "0,0,0,0")
    .split(",")
    .map(Number);
  return { left, top, width, height };
};

/** How far off a whole pixel a pane measures, as a fractional layout has it. */
let measuredOff = 0;

beforeEach(() => {
  measuredOff = 0;
  vi.useFakeTimers();
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
    let { left: x, top: y, width, height } = boxOf(this);
    if (this.hasAttribute("data-drop-pane")) width += measuredOff;
    if (this.getAttribute(TERMINAL_HOLDER) !== null && width === 0) {
      ({ left: x, top: y, width, height } = boxOf(this.parentElement as Element));
    }
    return DOMRect.fromRect({ x, y, width, height });
  });
  // The strip and the rows are found by the point; the panes never are: their zones stay put.
  document.elementFromPoint = (x: number, y: number) => {
    const holding = [
      ...document.querySelectorAll(`[${STRIP}], [${TAB}]:not([${"data-drop-pane"}]), [${RAIL}]`),
    ].filter((el) => {
      const b = boxOf(el);
      return x >= b.left && x < b.left + b.width && y >= b.top && y < b.top + b.height;
    });
    return holding[holding.length - 1] ?? null;
  };
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.useRealTimers();
});

function shown(props: { room?: typeof ROOM; active?: boolean } = {}) {
  let last = start();
  let tells = 0;
  let change: (how: (tabs: Tabs) => Tabs) => Tabs = () => last;
  const { unmount } = render(
    <Window
      first={last}
      heard={(tabs) => {
        last = tabs;
        tells += 1;
      }}
      control={(given) => (change = given)}
      {...props}
    />,
  );
  return {
    /** The tabs as the window last told them. */
    tabs: () => last,
    /** How many times the window has told its tabs. */
    tells: () => tells,
    change: (how: (tabs: Tabs) => Tabs) => change(how),
    unmount,
  };
}

/** The tab drawn in front on the strip, whatever the window has told. */
const drawnFront = () =>
  Number(
    document.querySelector('[role="tab"][aria-selected="true"]')?.parentElement?.getAttribute(TAB),
  );

/** ⌘⇧M, or Ctrl+Shift+M off a Mac: the registry's own chord, on what has the keyboard. */
function pressMove() {
  const chord = shortcut("pane.move").chords[onAMac() ? "mac" : "other"][0];
  act(() => {
    (document.activeElement ?? document.body).dispatchEvent(keystroke(chord));
  });
}

const key = (name: string) =>
  act(() => void fireEvent.keyDown(document.activeElement ?? document.body, { key: name }));

const at = (x: number, y: number) => ({ clientX: x, clientY: y, button: 0, isPrimary: true });
const frame = (session: number) => screen.getByTestId(`frame ${session}`);
const head = (session: number) => screen.getByText(`head ${session}`);
const heard = () => document.querySelector("[data-pane-drag-said]")?.textContent;
const settle = () => act(() => vi.advanceTimersByTime(70));

/** Where a pane is drawn now: where the window laid it out, moved and sized as a drag wrote. */
function drawn(session: number) {
  const one = frame(session);
  const box = boxOf(one);
  const moved = /translate\((-?[\d.]+)px, (-?[\d.]+)px\)/.exec(one.style.transform);
  return {
    left: box.left + Number(moved?.[1] ?? 0),
    top: box.top + Number(moved?.[2] ?? 0),
    width: one.style.width === "" ? box.width : parseFloat(one.style.width),
    height: one.style.height === "" ? box.height : parseFloat(one.style.height),
  };
}

const LEFT_HALF = { left: 0, top: 40, width: 800, height: 600 };
const RIGHT_HALF = { left: 800, top: 40, width: 800, height: 600 };
const part = (name: string) =>
  document.querySelector(`[data-scope="pane-preview"][data-part="${name}"]`);
const allParts = (name: string) => [
  ...document.querySelectorAll(`[data-scope="pane-preview"][data-part="${name}"]`),
];
const root = () => document.documentElement;

describe("the panes move to where a drop would leave them (#5291)", () => {
  it("swaps the sides for a pane held at another's left edge, before the drop, with the tabs as they were", () => {
    const window = shown();
    const before = window.tabs();
    fireEvent.pointerDown(head(3), at(900, 50));
    fireEvent.pointerMove(document, at(60, 300));
    settle();
    expect(drawn(3)).toEqual(LEFT_HALF);
    expect(drawn(1)).toEqual(RIGHT_HALF);
    expect(screen.getByText("chat 3 left of chat 1.")).toBeTruthy();
    expect(window.tabs()).toBe(before);
    fireEvent.pointerUp(document, at(60, 300));
    expect(window.tabs()).not.toBe(before);
  });

  it.each([
    [
      "top",
      400,
      50,
      { left: 0, top: 40, width: 1600, height: 300 },
      { left: 0, top: 340, width: 1600, height: 300 },
    ],
    [
      "bottom",
      400,
      630,
      { left: 0, top: 340, width: 1600, height: 300 },
      { left: 0, top: 40, width: 1600, height: 300 },
    ],
    ["middle", 400, 340, LEFT_HALF, RIGHT_HALF],
  ])("draws a pane held at another's %s", (_, x, y, moved, other) => {
    const window = shown();
    const before = window.tabs();
    fireEvent.pointerDown(head(3), at(900, 50));
    fireEvent.pointerMove(document, at(x, y));
    settle();
    expect(drawn(3)).toEqual(moved);
    expect(drawn(1)).toEqual(other);
    expect(window.tabs()).toBe(before);
  });

  it("draws a chat from the list as a pane arriving, with room made for it", () => {
    const window = shown();
    const before = window.tabs();
    fireEvent.pointerDown(screen.getByText("chat 7"), at(10, 705));
    fireEvent.pointerMove(document, at(1590, 340));
    settle();
    const arriving = part("arriving") as HTMLElement;
    expect(arriving.textContent).toBe("chat 7");
    expect(arriving.style.left).toBe("1200px");
    expect(arriving.style.width).toBe("400px");
    expect(drawn(1)).toEqual(LEFT_HALF);
    expect(drawn(3)).toEqual({ left: 800, top: 40, width: 400, height: 600 });
    expect(window.tabs()).toBe(before);
  });

  it("shows a chat from the list in a pane's place, and says the old chat keeps running", () => {
    shown();
    fireEvent.pointerDown(screen.getByText("chat 7"), at(10, 705));
    fireEvent.pointerMove(document, at(1200, 340));
    settle();
    const arriving = part("arriving") as HTMLElement;
    expect(arriving.style.left).toBe("800px");
    expect(arriving.style.width).toBe("800px");
    expect(frame(3).getAttribute("data-pane-preview")).toBe("gone");
    expect(screen.getByText("Show chat 7 here. chat 3 keeps running in the sidebar.")).toBeTruthy();
  });

  it("leaves the panes where they are on the strip, with a line where the tab would go", () => {
    shown();
    fireEvent.pointerDown(head(3), at(900, 50));
    fireEvent.pointerMove(document, at(150, 10));
    settle();
    expect(drawn(1)).toEqual(LEFT_HALF);
    expect(drawn(3)).toEqual(RIGHT_HALF);
    expect(part("line")).not.toBeNull();
  });

  it("dims a pane as it is picked up, and its row picks up the pane a chat is on screen in", () => {
    const window = shown();
    fireEvent.pointerDown(head(3), at(900, 50));
    fireEvent.pointerMove(document, at(900, 300));
    settle();
    expect(frame(3).getAttribute("data-pane-preview")).toBe("lifted");
    fireEvent.pointerMove(document, at(60, 300));
    settle();
    expect(frame(3).getAttribute("data-pane-preview")).toBe("moved");
    expect(window.tabs().inFront).toBeDefined();
  });

  it("draws nothing over the pane being moved, and says letting go leaves it", () => {
    shown();
    fireEvent.pointerDown(head(3), at(900, 50));
    fireEvent.pointerMove(document, at(1200, 340));
    settle();
    expect(drawn(3)).toEqual(RIGHT_HALF);
    expect(part("refused")).toBeNull();
    expect(root().hasAttribute("data-pane-refused")).toBe(false);
    const words = screen.getByText("Let go to leave it where it is.");
    expect(words.hasAttribute("data-refused")).toBe(false);
  });

  it("marks a refused place on its own pane, leaves the layout, and gives the no-drop cursor", () => {
    shown({ room: { left: 0, top: 40, width: 800, height: 600 } });
    fireEvent.pointerDown(head(3), at(500, 50));
    fireEvent.pointerMove(document, at(10, 340));
    settle();
    const mark = part("refused") as HTMLElement;
    expect(mark.style.left).toBe("0px");
    expect(mark.style.width).toBe("400px");
    expect(drawn(3)).toEqual({ left: 400, top: 40, width: 400, height: 600 });
    expect(root().hasAttribute("data-pane-refused")).toBe(true);
    const words = part("words") as HTMLElement;
    expect(words.textContent).toContain("Too narrow");
    expect(words.hasAttribute("data-refused")).toBe(true);
  });

  it("says letting go cancels over no place, and a label follows the pointer with what is held", () => {
    shown();
    fireEvent.pointerDown(head(3), at(900, 50));
    fireEvent.pointerMove(document, at(100, 900));
    settle();
    expect(screen.getByText("Let go here to cancel. Esc also cancels.")).toBeTruthy();
    const held = part("held") as HTMLElement;
    expect(held.textContent).toBe("chat 3");
    expect(held.style.left).toBe("114px");
    fireEvent.pointerMove(document, at(140, 900));
    expect(held.style.left).toBe("154px");
  });

  it("says why a side folded to a rail takes no drop, as a refusal, and letting go there cancels (#5415)", () => {
    const window = shown();
    fireEvent.pointerDown(head(3), at(900, 50));
    fireEvent.pointerMove(document, at(20, 1000));
    settle();
    expect(part("words")?.textContent).toBe(
      "A folded pane takes no drop. Let go here to cancel. Esc also cancels.",
    );
    expect(root().hasAttribute("data-pane-refused")).toBe(true);
    fireEvent.pointerUp(document, at(20, 1000));
    expect(window.tells()).toBe(0);
    expect(heard()).toBe("Put back. Nothing moved.");
  });
});

describe("a pane the drop leaves alone (#5291)", () => {
  it("stays where it is, with no label, though the layout measures off a whole pixel", () => {
    measuredOff = 0.4;
    shown();
    fireEvent.pointerDown(screen.getByText("chat 7"), at(10, 705));
    fireEvent.pointerMove(document, at(1590, 340));
    settle();
    expect(frame(1).style.transform).toBe("");
    expect(frame(1).style.width).toBe("");
    // Chat 3, made room in, and the chat arriving.
    expect(allParts("size")).toHaveLength(2);
  });
});

describe("the zones stay on the layout from the drag's start (#5291)", () => {
  it("holds the zone as the panes move: the same point stays the same place, whatever is drawn there", () => {
    shown();
    fireEvent.pointerDown(head(3), at(900, 50));
    fireEvent.pointerMove(document, at(60, 300));
    settle();
    // The pane held is drawn there now; the zone is still pane 1's left.
    expect(drawn(3)).toEqual(LEFT_HALF);
    fireEvent.pointerMove(document, at(61, 301));
    settle();
    expect(screen.getByText("chat 3 left of chat 1.")).toBeTruthy();
    // Past the old line between the panes is pane 3's own zone, even though pane 1 is drawn there.
    fireEvent.pointerMove(document, at(900, 300));
    settle();
    expect(screen.getByText("Let go to leave it where it is.")).toBeTruthy();
    expect(drawn(1)).toEqual(LEFT_HALF);
  });

  it("holds a place a little past its line", () => {
    shown();
    fireEvent.pointerDown(head(3), at(900, 50));
    fireEvent.pointerMove(document, at(150, 340));
    settle();
    expect(screen.getByText("chat 3 left of chat 1.")).toBeTruthy();
    // 27% of the pane's width: past the edge's quarter, inside its hold.
    fireEvent.pointerMove(document, at(216, 340));
    settle();
    expect(screen.getByText("chat 3 left of chat 1.")).toBeTruthy();
    fireEvent.pointerMove(document, at(260, 340));
    settle();
    expect(screen.getByText("Swap chat 3 and chat 1.")).toBeTruthy();
  });

  it("waits before a target changes, so a pointer crossing a pane shuffles nothing", () => {
    shown();
    fireEvent.pointerDown(head(3), at(900, 50));
    fireEvent.pointerMove(document, at(60, 300));
    act(() => vi.advanceTimersByTime(69));
    expect(drawn(3)).toEqual(RIGHT_HALF);
    fireEvent.pointerMove(document, at(100, 50));
    act(() => vi.advanceTimersByTime(69));
    expect(drawn(3)).toEqual(RIGHT_HALF);
    act(() => vi.advanceTimersByTime(1));
    expect(drawn(3)).toEqual({ left: 0, top: 40, width: 1600, height: 300 });
  });

  it("drops on the place under the pointer at the drop, waiting or not", () => {
    const window = shown();
    const front = window.tabs().order[0];
    fireEvent.pointerDown(head(3), at(900, 50));
    fireEvent.pointerMove(document, at(60, 300));
    fireEvent.pointerUp(document, at(60, 300));
    expect(
      contentsOf(window.tabs(), front).map(
        ({ content }) => content.kind === "session" && content.session,
      ),
    ).toEqual([3, 1]);
  });

  it("retakes the zones when another tab comes to the front", () => {
    const window = shown();
    const [, second] = window.tabs().order;
    fireEvent.pointerDown(head(3), at(900, 50));
    fireEvent.pointerMove(document, at(60, 300));
    settle();
    act(() => void window.change((t) => selectTab(t, second)));
    // A tab of one pane: the whole room is its zone now, and chat 3 goes beside it.
    fireEvent.pointerMove(document, at(1590, 340));
    settle();
    expect(screen.getByText("chat 3 right of chat 4.")).toBeTruthy();
    expect(drawn(4)).toEqual(LEFT_HALF);
    // Its terminal said how it sits only after it was measured, as a pane just drawn does.
    expect(allParts("size").map((one) => one.textContent)).toEqual(["80 × 27", "80 × 27"]);
  });

  it("measures nothing again for a change that leaves the same panes in the same places", () => {
    const window = shown();
    const front = window.tabs().order[0];
    const one = contentsOf(window.tabs(), front)[0].pane;
    fireEvent.pointerDown(head(3), at(900, 50));
    fireEvent.pointerMove(document, at(60, 300));
    settle();
    const measures = vi.mocked(Element.prototype.getBoundingClientRect);
    measures.mockClear();
    // What a pane shows in place of its chat draws another tree, of the same panes.
    act(
      () =>
        void window.change((t) => ({
          ...t,
          byId: { ...t.byId, [front]: { ...t.byId[front], shows: { [one]: 9 } } },
        })),
    );
    expect(
      measures.mock.contexts.filter((on) => (on as Element).hasAttribute("data-drop-pane")),
    ).toEqual([]);
  });

  it("says a target that closed has closed, and the preview goes back", () => {
    const window = shown();
    const front = window.tabs().order[0];
    const one = contentsOf(window.tabs(), front)[0].pane;
    fireEvent.pointerDown(head(3), at(900, 50));
    fireEvent.pointerMove(document, at(60, 300));
    settle();
    act(() => void window.change((t) => closePane(t, front, one, ctx.filedIn)));
    expect(screen.getByText("That pane has closed.")).toBeTruthy();
    expect(screen.queryByTestId("frame 1")).toBeNull();
    expect(frame(3).style.transform).toBe("");
  });
});

describe("cancelling restores exactly (#5291)", () => {
  /** A press on a pane's head and a move into a preview, with the keyboard on the field. */
  function dragging() {
    const window = shown();
    const front = window.tabs().order[0];
    const field = screen.getByLabelText("elsewhere");
    (field as HTMLElement).focus();
    const before = window.tabs();
    fireEvent.pointerDown(head(3), at(900, 50));
    fireEvent.pointerMove(document, at(60, 300));
    settle();
    expect(drawn(3)).toEqual(LEFT_HALF);
    return { window, front, before, field };
  }

  const nothingLeft = () => {
    for (const session of [1, 3]) {
      const one = frame(session);
      expect(one.style.transform).toBe("");
      expect(one.style.width).toBe("");
      expect(one.style.height).toBe("");
      expect(one.hasAttribute("data-pane-preview")).toBe(false);
    }
    for (const holder of screen.getAllByTestId("holder")) {
      expect(holder.style.width).toBe("");
      expect(holder.style.height).toBe("");
    }
    expect(root().hasAttribute("data-pane-dragging")).toBe(false);
    expect(root().hasAttribute("data-pane-previewing")).toBe(false);
    expect(root().hasAttribute("data-pane-refused")).toBe(false);
    expect(part("arriving")).toBeNull();
    expect(part("size")).toBeNull();
    expect(part("words")).toBeNull();
    expect(part("held")).toBeNull();
  };

  it.each([
    ["Esc", () => fireEvent.keyDown(document, { key: "Escape" })],
    ["a release outside any place", () => fireEvent.pointerUp(document, at(100, 900))],
    ["the window losing focus", () => fireEvent.blur(window)],
    ["the pointer being taken away", () => fireEvent.pointerCancel(document, at(60, 300))],
  ])("puts everything back on %s", async (_, cancel) => {
    const { window: win, before } = dragging();
    cancel();
    expect(win.tabs()).toBe(before);
    nothingLeft();
    act(() => vi.advanceTimersByTime(1));
    expect(heard()).toBe("Put back. Nothing moved.");
  });

  it("drops nothing once it is cancelled, when the pointer comes up later", () => {
    const { window: win, before } = dragging();
    fireEvent.keyDown(document, { key: "Escape" });
    fireEvent.pointerUp(document, at(60, 300));
    act(() => vi.advanceTimersByTime(100));
    expect(win.tabs()).toBe(before);
  });

  it("gives the keyboard back to what had it", () => {
    const { field } = dragging();
    screen.getByText("chat 7").focus();
    fireEvent.keyDown(document, { key: "Escape" });
    act(() => vi.advanceTimersByTime(1));
    expect(document.activeElement).toBe(field);
  });

  it("gives the keyboard to the focused pane when what had it has gone", () => {
    const window = shown();
    const [, second] = window.tabs().order;
    screen.getByText("Close 3").focus();
    fireEvent.pointerDown(head(3), at(900, 50));
    fireEvent.pointerMove(document, at(60, 300));
    settle();
    act(() => void window.change((t) => selectTab(t, second)));
    fireEvent.keyDown(document, { key: "Escape" });
    act(() => vi.advanceTimersByTime(1));
    expect(document.activeElement).toBe(screen.getByText("Close 3"));
  });

  it("puts the focused pane back, when the press focused the pane it took", () => {
    const window = shown();
    const front = window.tabs().order[0];
    const [one, three] = contentsOf(window.tabs(), front).map(({ pane }) => pane);
    act(() => void window.change((t) => focusPane(t, one)));
    fireEvent.pointerDown(head(3), at(900, 50));
    act(() => void window.change((t) => focusPane(t, three)));
    fireEvent.pointerMove(document, at(60, 300));
    settle();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(window.tabs().byId[front].focused).toBe(one);
  });
});

describe("a terminal keeps its size until the drop (#5291)", () => {
  it("pins each terminal's holder as a drag starts, whatever its frame does", () => {
    shown();
    fireEvent.pointerDown(head(3), at(900, 50));
    fireEvent.pointerMove(document, at(60, 50));
    settle();
    expect(drawn(3)).toEqual({ left: 0, top: 40, width: 1600, height: 300 });
    for (const holder of screen.getAllByTestId("holder")) {
      expect(holder.style.width).toBe("800px");
      expect(holder.style.height).toBe("540px");
    }
    fireEvent.pointerUp(document, at(60, 50));
    for (const holder of screen.getAllByTestId("holder")) expect(holder.style.width).toBe("");
  });

  it("labels each pane that will change with its size after the drop, and no other", () => {
    shown();
    fireEvent.pointerDown(head(3), at(900, 50));
    fireEvent.pointerMove(document, at(60, 50));
    settle();
    expect(allParts("size").map((one) => one.textContent)).toEqual(["160 × 12", "160 × 12"]);
    fireEvent.pointerMove(document, at(60, 300));
    settle();
    // The two change places at the same size: no terminal will change.
    expect(allParts("size")).toEqual([]);
    fireEvent.keyDown(document, { key: "Escape" });
  });

  it("labels a pane arriving by the cells of the terminals on screen", () => {
    shown();
    fireEvent.pointerDown(screen.getByText("chat 7"), at(10, 705));
    fireEvent.pointerMove(document, at(1590, 340));
    settle();
    // Chat 3 at half its width, and the chat arriving beside it.
    expect(allParts("size").map((one) => one.textContent)).toEqual(["40 × 27", "40 × 27"]);
  });

  it("pins a pane's top, so a notice arriving waits on the bell", () => {
    shown();
    const top = frame(3).querySelector<HTMLElement>(".pane-top");
    fireEvent.pointerDown(head(3), at(900, 50));
    fireEvent.pointerMove(document, at(60, 300));
    expect(top?.style.height).toBe("20px");
    fireEvent.pointerUp(document, at(60, 300));
  });
});

describe("a project behind the one in front (#5291)", () => {
  it("hears no press: it never starts a drag of its own", () => {
    shown({ active: false });
    fireEvent.pointerDown(head(3), at(900, 50));
    fireEvent.pointerMove(document, at(60, 300));
    settle();
    expect(part("words")).toBeNull();
    expect(root().hasAttribute("data-pane-dragging")).toBe(false);
  });
});

describe("resting on a tab during a drag brings it forward, as a preview (#5294)", () => {
  /** A pane picked up and rested on the second tab until it comes forward. */
  function restedOn() {
    const window = shown();
    const [first, second] = window.tabs().order;
    fireEvent.pointerDown(head(3), at(900, 50));
    fireEvent.pointerMove(document, at(150, 10));
    act(() => vi.advanceTimersByTime(HOLD_MS));
    expect(drawnFront()).toBe(second);
    return { window, first, second };
  }

  it("brings the tab rested on forward once the time is up, drawn and told to nobody", () => {
    const window = shown();
    const [first, second] = window.tabs().order;
    const before = window.tabs();
    fireEvent.pointerDown(head(3), at(900, 50));
    fireEvent.pointerMove(document, at(150, 10));
    act(() => vi.advanceTimersByTime(HOLD_MS - 1));
    expect(drawnFront()).toBe(first);
    act(() => vi.advanceTimersByTime(1));
    expect(drawnFront()).toBe(second);
    expect(window.tabs()).toBe(before);
    expect(window.tells()).toBe(0);
  });

  it("brings nothing forward when the pointer moves off the tab before the time", () => {
    const window = shown();
    const [first] = window.tabs().order;
    fireEvent.pointerDown(head(3), at(900, 50));
    fireEvent.pointerMove(document, at(150, 10));
    act(() => vi.advanceTimersByTime(HOLD_MS - 1));
    fireEvent.pointerMove(document, at(60, 300));
    act(() => vi.advanceTimersByTime(HOLD_MS));
    expect(drawnFront()).toBe(first);
  });

  it("marks the tab while it is about to come forward, and not the tab already in front", () => {
    shown();
    fireEvent.pointerDown(head(3), at(900, 50));
    fireEvent.pointerMove(document, at(50, 10));
    act(() => vi.advanceTimersByTime(HOLD_MS));
    expect(part("opening")).toBeNull();
    fireEvent.pointerMove(document, at(150, 10));
    act(() => vi.advanceTimersByTime(1));
    const mark = part("opening") as HTMLElement | null;
    expect(mark).not.toBeNull();
    // Along the bottom of the second tab, as the strip draws it at 100, 0, 100 × 30.
    expect([mark?.style.left, mark?.style.width]).toEqual(["100px", "100px"]);
    act(() => vi.advanceTimersByTime(HOLD_MS));
    expect(part("opening")).toBeNull();
  });

  it("retakes the zones for the tab now in front, and a drop lands in it, told once", () => {
    const { window, first, second } = restedOn();
    fireEvent.pointerMove(document, at(1590, 340));
    settle();
    expect(screen.getByText("chat 3 right of chat 4.")).toBeTruthy();
    fireEvent.pointerUp(document, at(1590, 340));
    expect(window.tells()).toBe(1);
    const sessions = (tab: number) =>
      contentsOf(window.tabs(), tab).map(({ content }) =>
        content.kind === "session" ? content.session : undefined,
      );
    expect(window.tabs().inFront).toBe(second);
    expect(sessions(second)).toEqual([4, 3]);
    expect(sessions(first)).toEqual([1]);
  });

  it("returns to the tab that was in front on Esc, and has told nothing", () => {
    const { window, first } = restedOn();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(drawnFront()).toBe(first);
    expect(window.tells()).toBe(0);
    expect(frame(3).style.transform).toBe("");
  });

  it("puts the tab back and tells nothing when the window goes mid-drag", () => {
    const { window, first } = restedOn();
    window.unmount();
    expect(window.tells()).toBe(0);
    expect(window.tabs().inFront).toBe(first);
    expect(root().hasAttribute("data-pane-dragging")).toBe(false);
  });

  it("closes the tab whose last pane leaves for the tab rested on", () => {
    const window = shown();
    const [first, second] = window.tabs().order;
    act(() => void window.change((t) => selectTab(t, second)));
    fireEvent.pointerDown(head(4), at(100, 50));
    fireEvent.pointerMove(document, at(50, 10));
    act(() => vi.advanceTimersByTime(HOLD_MS));
    expect(drawnFront()).toBe(first);
    fireEvent.pointerMove(document, at(1590, 340));
    settle();
    fireEvent.pointerUp(document, at(1590, 340));
    expect(window.tabs().order).toEqual([first]);
    expect(
      contentsOf(window.tabs(), first).map(({ content }) =>
        content.kind === "session" ? content.session : undefined,
      ),
    ).toEqual([1, 3, 4]);
  });
});

describe("placement mode draws the same preview as a drag (#5293)", () => {
  /** Chat 3 picked up with ⌘⇧M from its pane, the keyboard on the first target. */
  function picked() {
    const window = shown();
    const close = screen.getByText("Close 3");
    close.focus();
    pressMove();
    act(() => vi.advanceTimersByTime(1));
    return { window, close };
  }
  const words = () => part("words")?.textContent;
  const onTarget = () => document.activeElement?.getAttribute("aria-label");

  it("draws the preview of the place the keyboard is on, with its words, at each step", () => {
    const { window } = picked();
    const before = window.tabs();
    // The keyboard starts on chat 1's middle: the two change places.
    expect(onTarget()).toBe("Swap chat 3 and chat 1.");
    expect(words()).toBe("Swap chat 3 and chat 1.");
    expect(drawn(3)).toEqual(LEFT_HALF);
    expect(drawn(1)).toEqual(RIGHT_HALF);
    key("ArrowDown");
    expect(words()).toBe(onTarget());
    expect(drawn(1)).toEqual({ left: 0, top: 40, width: 1600, height: 300 });
    expect(drawn(3)).toEqual({ left: 0, top: 340, width: 1600, height: 300 });
    expect(allParts("size").map((one) => one.textContent)).toEqual(["160 × 12", "160 × 12"]);
    expect(window.tabs()).toBe(before);
    expect(root().getAttribute("data-pane-dragging")).toBe("keys");
  });

  it("drops on Enter where the preview drew it, and lets go of the panes", () => {
    const { window } = picked();
    key("ArrowDown");
    key("Enter");
    const front = window.tabs().inFront as number;
    expect(window.tabs().byId[front].layout).toMatchObject({ direction: "column" });
    expect(frame(3).style.transform).toBe("");
    expect(part("words")).toBeNull();
    expect(root().hasAttribute("data-pane-dragging")).toBe(false);
  });

  it("brings back the tab in front on Esc, whatever brought another forward", () => {
    const { window } = picked();
    const [first, second] = window.tabs().order;
    act(() => void window.change((t) => selectTab(t, second)));
    key("Escape");
    expect(drawnFront()).toBe(first);
    expect(window.tabs().inFront).toBe(first);
  });

  it("restores sizes, the front tab and the keyboard on Esc, having told nothing", () => {
    const { window, close } = picked();
    const [first, second] = window.tabs().order;
    fireEvent.pointerDown(screen.getByText(`tab ${second}`), at(150, 10));
    fireEvent.pointerUp(screen.getByText(`tab ${second}`), at(150, 10));
    act(() => vi.advanceTimersByTime(1));
    expect(drawnFront()).toBe(second);
    // The tab in front is the one whose panes take it now.
    expect(onTarget()).toBe("Swap chat 3 and chat 4.");
    key("Escape");
    act(() => vi.advanceTimersByTime(1));
    expect(drawnFront()).toBe(first);
    expect(window.tells()).toBe(0);
    for (const session of [1, 3]) expect(frame(session).style.transform).toBe("");
    expect(part("words")).toBeNull();
    // Its pane is drawn again with its tab, and its own control has the keyboard there.
    expect(close.isConnected).toBe(false);
    expect(document.activeElement).toBe(screen.getByText("Close 3"));
  });
});
