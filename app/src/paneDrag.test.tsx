// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useRef, useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { UNDO_MS } from "./MemoryEdits";
import { CHAT_ROW, dropPane, GRIP, HOLD_MS, STRIP, TAB, usePaneDrag } from "./paneDrag";
import type { DropContext } from "./paneDrop";
import { recorded } from "./layoutUndo";
import {
  contentsOf,
  insertPane,
  type Layout,
  noTabs,
  openTab,
  selectTab,
  splitFocusedPane,
  type Tabs,
} from "./tabs";

/**
 * **Dragging panes in the window** (#4763): the pointer half of `paneDrop.ts`, which decides
 * every drop (`paneDrop.test.ts`). jsdom lays nothing out, so each element says its own box
 * (`data-box`) and the point under the pointer is found from those: what is tested is what the
 * window does with a place, never how WebKit measures one (a hardware follow-up).
 */

const ctx: DropContext = {
  filedIn: () => "ops",
  name: (content) => (content.kind === "session" ? `chat ${content.session}` : "the empty pane"),
};

/** One tab: chat 1 left, chat 3 right, each 800 × 600; then a second tab, chat 4. */
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

function shape(layout: Layout): string {
  if (layout.kind === "pane")
    return layout.content.kind === "session" ? String(layout.content.session) : "-";
  return `${layout.direction}(${shape(layout.children[0])} ${shape(layout.children[1])})`;
}

/** What the window does with its tabs, as far as a drag reaches them. */
function Window({
  first,
  heard,
  width = 800,
}: {
  first: Tabs;
  heard: (tabs: Tabs) => void;
  width?: number;
}) {
  const [tabs, setTabs] = useState(first);
  const now = useRef(first);
  const change = useCallback(
    (how: (tabs: Tabs) => Tabs) => {
      const next = how(now.current);
      now.current = next;
      setTabs(next);
      heard(next);
      return next;
    },
    [heard],
  );
  const hold = useCallback((id: number) => change((t) => selectTab(t, id)), [change]);
  const own = useCallback((session: number) => String(session), []);
  const drag = usePaneDrag({ tabs, change, ctx, onHoldTab: hold, chatNameOf: own });
  const front = tabs.inFront ?? tabs.order[0];
  const panes = contentsOf(tabs, front);
  return (
    <div>
      <div {...{ [STRIP]: "" }} data-box="0,0,1600,30">
        {tabs.order.map((id, at) => (
          <div key={id} {...{ [TAB]: String(id) }} data-box={`${at * 100},0,100,30`}>
            <span role="tab" aria-selected={id === tabs.inFront}>{`tab ${id}`}</span>
            <button type="button" aria-label={`Close tab ${id}`}>
              ×
            </button>
          </div>
        ))}
      </div>
      {panes.map(({ pane }, at) => (
        <div key={pane} {...dropPane(front, pane)} data-box={`${at * width},40,${width},600`}>
          <div {...{ [GRIP]: "" }} data-box={`${at * width},40,${width},20`}>
            {`head ${pane}`}
          </div>
        </div>
      ))}
      <button type="button" {...{ [CHAT_ROW]: "7" }} data-box="0,700,200,20">
        chat 7
      </button>
      <button
        type="button"
        onClick={() =>
          change((t) => recorded(t, splitFocusedPane(t, "column", 9, "9"), "Split the pane."))
        }
      >
        Split down
      </button>
      {drag.layer}
      {drag.undoLine}
    </div>
  );
}

const boxOf = (el: Element) => {
  const [left, top, width, height] = (el.getAttribute("data-box") ?? "0,0,0,0")
    .split(",")
    .map(Number);
  return { left, top, width, height, right: left + width, bottom: top + height, x: left, y: top };
};

beforeEach(() => {
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
    // A real one, as a browser measures: its sides are getters on its prototype, so a spread of
    // it gives nothing, and no test here passes by reading a plain object (#5284).
    const { left: x, top: y, width, height } = boxOf(this);
    return DOMRect.fromRect({ x, y, width, height });
  });
  // The deepest element whose box holds the point.
  document.elementFromPoint = (x: number, y: number) => {
    const holding = [...document.querySelectorAll("[data-box]")].filter((el) => {
      const b = boxOf(el);
      return x >= b.left && x < b.right && y >= b.top && y < b.bottom;
    });
    return holding[holding.length - 1] ?? null;
  };
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.useRealTimers();
});

function shown(width?: number): { tabs: () => Tabs } {
  let last = start();
  render(<Window first={last} heard={(tabs) => (last = tabs)} width={width} />);
  return { tabs: () => last };
}

/** What a reader heard last of the drag. */
const heard = () => document.querySelector("[data-pane-drag-said]")?.textContent;
const head = (n: number) => screen.getByText(`head ${n}`);
const at = (x: number, y: number) => ({ clientX: x, clientY: y, button: 0, isPrimary: true });

describe("dragging a pane in the window (#4763)", () => {
  it("draws the result before the drop, and drops on the place the pointer picked", async () => {
    const window = shown();
    const front = window.tabs().order[0];
    const three = contentsOf(window.tabs(), front)[1].pane;
    fireEvent.pointerDown(head(three), at(900, 50));
    fireEvent.pointerMove(document, at(60, 300));
    expect(await screen.findByText("chat 3 left of chat 1.")).toBeTruthy();
    expect(document.querySelector(".pane-drop-preview")?.getAttribute("style")).toContain(
      "width: 400px",
    );
    fireEvent.pointerUp(document, at(60, 300));
    expect(shape(window.tabs().byId[front].layout)).toBe("row(3 1)");
    await waitFor(() => expect(heard()).toBe("Put chat 3 left of chat 1."));
    expect(document.querySelector(".pane-drop-preview")).toBeNull();
  });

  it("swaps in a pane's middle", () => {
    const window = shown();
    const front = window.tabs().order[0];
    fireEvent.pointerDown(head(contentsOf(window.tabs(), front)[0].pane), at(100, 50));
    fireEvent.pointerMove(document, at(1200, 340));
    fireEvent.pointerUp(document, at(1200, 340));
    expect(shape(window.tabs().byId[front].layout)).toBe("row(3 1)");
  });

  it("puts it back on Esc, and nothing moves", async () => {
    const window = shown();
    const before = window.tabs();
    const front = before.order[0];
    fireEvent.pointerDown(head(contentsOf(before, front)[1].pane), at(900, 50));
    fireEvent.pointerMove(document, at(60, 300));
    fireEvent.keyDown(document, { key: "Escape" });
    fireEvent.pointerUp(document, at(60, 300));
    expect(window.tabs()).toBe(before);
    await waitFor(() => expect(heard()).toBe("Put back. Nothing moved."));
    expect(document.querySelector(".pane-drop-preview")).toBeNull();
  });

  it("is no drag at all for a press that does not move", () => {
    const window = shown();
    const before = window.tabs();
    fireEvent.pointerDown(head(contentsOf(before, before.order[0])[1].pane), at(900, 50));
    fireEvent.pointerUp(document, at(901, 51));
    expect(window.tabs()).toBe(before);
    expect(heard()).toBe("");
  });

  it("draws a refused place with its reason, and drops nothing there", async () => {
    // Each half of a pane 400 wide would be under the floor.
    const window = shown(400);
    const before = window.tabs();
    const front = before.order[0];
    const one = contentsOf(before, front)[0].pane;
    fireEvent.pointerDown(head(contentsOf(before, front)[1].pane), at(500, 50));
    fireEvent.pointerMove(document, at(10, 340));
    const why = "Too narrow: a pane here would be under about 40 columns. Drop it on a wider pane.";
    const said = await screen.findByText(why);
    expect(said.hasAttribute("data-refused")).toBe(true);
    fireEvent.pointerUp(document, at(10, 340));
    expect(window.tabs()).toBe(before);
    expect(contentsOf(window.tabs(), front)[0].pane).toBe(one);
    await waitFor(() => expect(heard()).toBe(`Not dropped: ${why}`));
  });

  it("offers Undo after a drop, which puts the last layout back", async () => {
    const window = shown();
    const front = window.tabs().order[0];
    fireEvent.pointerDown(head(contentsOf(window.tabs(), front)[1].pane), at(900, 50));
    fireEvent.pointerMove(document, at(60, 300));
    fireEvent.pointerUp(document, at(60, 300));
    expect(shape(window.tabs().byId[front].layout)).toBe("row(3 1)");
    fireEvent.click(await screen.findByRole("button", { name: "Undo" }));
    expect(shape(window.tabs().byId[front].layout)).toBe("row(1 3)");
    await waitFor(() => expect(screen.queryByRole("button", { name: "Undo" })).toBeNull());
  });

  it("puts the last layout back on ⌘Z", async () => {
    const window = shown();
    const front = window.tabs().order[0];
    fireEvent.pointerDown(head(contentsOf(window.tabs(), front)[1].pane), at(900, 50));
    fireEvent.pointerMove(document, at(60, 300));
    fireEvent.pointerUp(document, at(60, 300));
    await screen.findByRole("button", { name: "Undo" });
    fireEvent.keyDown(document.body, { key: "z", metaKey: true });
    fireEvent.keyDown(document.body, { key: "z", ctrlKey: true });
    expect(shape(window.tabs().byId[front].layout)).toBe("row(1 3)");
  });

  it("makes a tab of a sidebar chat dropped on the strip", () => {
    const window = shown();
    fireEvent.pointerDown(screen.getByText("chat 7"), at(10, 710));
    fireEvent.pointerMove(document, at(400, 10));
    fireEvent.pointerUp(document, at(400, 10));
    const tabs = window.tabs();
    expect(tabs.order.map((id) => shape(tabs.byId[id].layout))).toEqual(["row(1 3)", "4", "7"]);
  });

  it("splits a tab in beside a pane once it is pulled down off the strip, and not before", () => {
    const window = shown();
    const [first, second] = window.tabs().order;
    const escapes: string[] = [];
    const heardKey = (key: KeyboardEvent) => escapes.push(key.code);
    document.addEventListener("keydown", heardKey);
    fireEvent.pointerDown(screen.getByText(`tab ${second}`), at(150, 10));
    // Along the strip, and just under it, it is still the strip's own drag.
    fireEvent.pointerMove(document, at(400, 20));
    fireEvent.pointerMove(document, at(400, 50));
    expect(document.querySelector(".pane-drop-preview")).toBeNull();
    fireEvent.pointerMove(document, at(1500, 340));
    // The strip's drag is told to end as the tab leaves it.
    expect(escapes).toEqual(["Escape"]);
    fireEvent.pointerUp(document, at(1500, 340));
    document.removeEventListener("keydown", heardKey);
    const tabs = window.tabs();
    expect(tabs.order).toEqual([first]);
    expect(shape(tabs.byId[first].layout)).toBe("row(1 row(3 4))");
  });

  it("starts no drag from a control beside a tab, such as its close", () => {
    const window = shown();
    const before = window.tabs();
    const second = before.order[1];
    fireEvent.pointerDown(screen.getByRole("button", { name: `Close tab ${second}` }), at(190, 10));
    fireEvent.pointerMove(document, at(1500, 340));
    fireEvent.pointerUp(document, at(1500, 340));
    expect(window.tabs()).toBe(before);
    expect(document.querySelector(".pane-drop-preview")).toBeNull();
  });

  it("hides the Undo line after a few seconds, and ⌘Z still puts the layout back", () => {
    vi.useFakeTimers();
    const window = shown();
    const front = window.tabs().order[0];
    fireEvent.pointerDown(head(contentsOf(window.tabs(), front)[1].pane), at(900, 50));
    fireEvent.pointerMove(document, at(60, 300));
    fireEvent.pointerUp(document, at(60, 300));
    expect(screen.getByRole("button", { name: "Undo" })).toBeTruthy();
    act(() => vi.advanceTimersByTime(UNDO_MS));
    expect(screen.queryByRole("button", { name: "Undo" })).toBeNull();
    fireEvent.keyDown(document.body, { key: "z", metaKey: true });
    fireEvent.keyDown(document.body, { key: "z", ctrlKey: true });
    expect(shape(window.tabs().byId[front].layout)).toBe("row(1 3)");
  });

  it("offers the same Undo, toast and ⌘Z, after a split as after a drop", async () => {
    const window = shown();
    const front = window.tabs().order[0];
    fireEvent.click(screen.getByRole("button", { name: "Split down" }));
    expect(shape(window.tabs().byId[front].layout)).not.toBe("row(1 3)");
    await screen.findByText("Split the pane.");
    fireEvent.keyDown(document.body, { key: "z", metaKey: true });
    fireEvent.keyDown(document.body, { key: "z", ctrlKey: true });
    await waitFor(() => expect(shape(window.tabs().byId[front].layout)).toBe("row(1 3)"));
    expect(screen.queryByRole("button", { name: "Undo" })).toBeNull();
  });

  it("brings a tab forward when a drag holds over it", () => {
    vi.useFakeTimers();
    const window = shown();
    const [first, second] = window.tabs().order;
    fireEvent.pointerDown(head(contentsOf(window.tabs(), first)[1].pane), at(900, 50));
    fireEvent.pointerMove(document, at(150, 10));
    act(() => vi.advanceTimersByTime(HOLD_MS - 1));
    expect(window.tabs().inFront).toBe(first);
    act(() => vi.advanceTimersByTime(1));
    expect(window.tabs().inFront).toBe(second);
  });
});

describe("where a drag's preview is drawn (#5284)", () => {
  const preview = () => document.querySelector<HTMLElement>(".pane-drop-preview");
  const boxDrawn = () => {
    const { left, top, width, height } = preview()?.style ?? {};
    return { left, top, width, height };
  };

  it.each([
    ["left", 810, 340, { left: "800px", top: "40px", width: "400px", height: "600px" }],
    ["right", 1590, 340, { left: "1200px", top: "40px", width: "400px", height: "600px" }],
    ["top", 1200, 50, { left: "800px", top: "40px", width: "800px", height: "300px" }],
    ["bottom", 1200, 630, { left: "800px", top: "340px", width: "800px", height: "300px" }],
    ["middle", 1200, 340, { left: "800px", top: "40px", width: "800px", height: "600px" }],
  ])("draws a drop on a pane's %s over that part of the pane", (_, x, y, box) => {
    const window = shown();
    const front = window.tabs().order[0];
    fireEvent.pointerDown(head(contentsOf(window.tabs(), front)[0].pane), at(100, 50));
    fireEvent.pointerMove(document, at(x, y));
    expect(preview()?.hasAttribute("data-refused")).toBe(false);
    expect(boxDrawn()).toEqual(box);
  });

  it("draws a refused place dashed over the pane it refers to, never across the window's top", () => {
    const window = shown(400);
    const front = window.tabs().order[0];
    fireEvent.pointerDown(head(contentsOf(window.tabs(), front)[0].pane), at(100, 50));
    fireEvent.pointerMove(document, at(410, 340));
    expect(preview()?.hasAttribute("data-refused")).toBe(true);
    expect(boxDrawn()).toEqual({ left: "400px", top: "40px", width: "200px", height: "600px" });
  });

  it.each([
    ["its edge", 10, 340],
    ["its middle", 400, 340],
  ])(
    "draws no danger over the pane being moved, at %s, and letting go moves nothing",
    (_, x, y) => {
      const window = shown();
      const before = window.tabs();
      const front = before.order[0];
      fireEvent.pointerDown(head(contentsOf(before, front)[0].pane), at(100, 50));
      fireEvent.pointerMove(document, at(x, y));
      expect(preview()).toBeNull();
      const tip = screen.getByText("That's the pane being moved. Drop it on another pane.");
      expect(tip.hasAttribute("data-refused")).toBe(false);
      fireEvent.pointerUp(document, at(x, y));
      expect(window.tabs()).toBe(before);
      expect(preview()).toBeNull();
    },
  );

  it("says a pane let go over itself was put back, and not that a drop was refused", async () => {
    const window = shown();
    const front = window.tabs().order[0];
    fireEvent.pointerDown(head(contentsOf(window.tabs(), front)[0].pane), at(100, 50));
    fireEvent.pointerMove(document, at(400, 340));
    fireEvent.pointerUp(document, at(400, 340));
    await waitFor(() => expect(heard()).toBe("Put back. Nothing moved."));
  });

  it("draws no danger over a pane of the tab being moved", () => {
    const window = shown();
    const [first] = window.tabs().order;
    fireEvent.pointerDown(screen.getByText(`tab ${first}`), at(10, 10));
    fireEvent.pointerMove(document, at(400, 340));
    expect(preview()).toBeNull();
    const tip = screen.getByText("That's the tab being moved. Drop it on a pane in another tab.");
    expect(tip.hasAttribute("data-refused")).toBe(false);
  });
});
