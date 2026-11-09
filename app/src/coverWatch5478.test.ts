// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { AT_MOST_EVERY_MS, clockForTest, watchForCover } from "./coverWatch5478";

/**
 * [DEBUG-5478] **What takes a frozen pane's keys and mouse** (#5478): a press that lands in a
 * pane's terminal and reaches something else writes one line naming what is on top; a press
 * that reaches the terminal writes none.
 *
 * A pane here is what `SessionPane` draws: a holder with the terminal's `.xterm` in it. jsdom
 * lays nothing out, so the holder's box and `elementsFromPoint` are the test's.
 */
let holder: HTMLElement;
let xterm: HTMLElement;
let field: HTMLTextAreaElement;
let logged: string[];
let time: number;
let leave: () => void;
let unclock: () => void;
let stack: Element[];

const box = { left: 0, top: 0, right: 400, bottom: 300, width: 400, height: 300, x: 0, y: 0 };

function press(on: Element, x = 100, y = 100) {
  on.dispatchEvent(
    new MouseEvent("pointerdown", { bubbles: true, cancelable: true, clientX: x, clientY: y }),
  );
}
const key = (on: Element) =>
  on.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, key: "a" }));

function overlay(): HTMLElement {
  const layer = document.createElement("div");
  layer.className = "left-over-layer";
  layer.dataset.dragLayer = "";
  layer.style.zIndex = "40";
  document.body.append(layer);
  stack = [layer, xterm, holder, document.body];
  return layer;
}

const said = (line: string) =>
  JSON.parse(line.slice(line.indexOf("{"))) as Record<string, unknown> & {
    stack: Record<string, unknown>[];
  };

beforeEach(() => {
  document.body.innerHTML = "";
  holder = document.createElement("div");
  holder.dataset.paneTerminal = "";
  holder.getBoundingClientRect = () => box as DOMRect;
  xterm = document.createElement("div");
  xterm.className = "xterm";
  field = document.createElement("textarea");
  field.className = "xterm-helper-textarea";
  xterm.append(field);
  holder.append(xterm);
  document.body.append(holder);
  stack = [xterm, holder, document.body];
  document.elementsFromPoint = () => stack;
  logged = [];
  time = 1_000_000;
  unclock = clockForTest(() => time);
  leave = watchForCover(
    holder,
    7,
    () => ({ element: xterm }),
    (line) => logged.push(line),
  );
});
afterEach(() => {
  leave();
  unclock();
});

describe("[DEBUG-5478] what covers a pane", () => {
  it("writes nothing for a press that reaches the pane's terminal", () => {
    press(xterm);
    press(field);

    expect(logged).toEqual([]);
  });

  it("writes one line naming what is on top when something over the pane takes the press", () => {
    const layer = overlay();

    press(layer);

    expect(logged).toHaveLength(1);
    expect(logged[0]).toMatch(/^pane missed pointerdown: \{/);
    const line = said(logged[0]);
    expect(line).toMatchObject({
      session: 7,
      holderConnected: true,
      ownedConnected: true,
      ownedInHolder: true,
      xtermsInHolder: 1,
      shownIsOwned: true,
      at: { x: 100, y: 100 },
      target: { tag: "div", class: "left-over-layer", data: ["dragLayer"] },
      activeElement: { tag: "body" },
    });
    expect(line.stack).toHaveLength(4);
    expect(line.stack[0]).toMatchObject({ class: "left-over-layer", zIndex: "40" });
    expect(line.stack[0]).toHaveProperty("pointerEvents");
    expect(line.stack[0]).toHaveProperty("opacity");
  });

  it("writes nothing for a press outside the pane, covered or not", () => {
    const layer = overlay();

    press(layer, 500, 100);

    expect(logged).toEqual([]);
  });

  it("writes at most one line a minute for a pane that stays covered", () => {
    const layer = overlay();

    for (let second = 0; second < 59; second += 5) {
      press(layer);
      time += 5_000;
    }
    expect(logged).toHaveLength(1);

    time += 5_000;
    press(layer);
    expect(logged).toHaveLength(2);
  });

  it("says when a press reaches the terminal again, and how long it was missed", () => {
    const layer = overlay();
    press(layer);
    time += 12_300;
    layer.remove();
    stack = [xterm, holder, document.body];

    press(xterm);
    press(xterm);

    expect(logged).toHaveLength(2);
    expect(logged[1]).toBe(
      'pane recovered after 12.3 s: {"session":7,"by":"pointerdown","missedMs":12300}',
    );
  });

  it("writes no recovery line for a miss whose own line the minute held back", () => {
    const layer = overlay();
    press(layer);
    press(xterm);
    expect(logged).toHaveLength(2);

    time += 5_000;
    press(layer);
    press(xterm);

    expect(logged).toHaveLength(2);
  });

  it("says the pane shows a terminal that is not its live one", () => {
    // A stale copy in the holder, and the live terminal's element in no document.
    const stale = document.createElement("div");
    stale.className = "xterm";
    holder.replaceChildren(stale);
    stack = [stale, holder, document.body];

    press(stale);

    expect(logged).toHaveLength(1);
    expect(said(logged[0])).toMatchObject({
      ownedConnected: false,
      ownedInHolder: false,
      xtermsInHolder: 1,
      shownIsOwned: false,
    });
  });

  it("writes a line for a key typed with the keyboard in no terminal after a press on one", () => {
    const layer = overlay();
    press(layer);
    time += AT_MOST_EVERY_MS;
    (document.activeElement as HTMLElement | null)?.blur();

    key(document.body);

    expect(logged).toHaveLength(2);
    expect(logged[1]).toMatch(/^pane missed keydown: \{/);
    expect(said(logged[1])).toMatchObject({ session: 7, at: { x: 200, y: 150 } });
  });

  it("writes nothing for a key the terminal has, or one typed in a field", () => {
    press(xterm);
    field.focus();
    key(field);
    const input = document.createElement("input");
    document.body.append(input);
    input.focus();
    key(input);

    expect(logged).toEqual([]);
  });

  it("writes nothing once the pane has gone", () => {
    const layer = overlay();
    leave();

    press(layer);

    expect(logged).toEqual([]);
  });
});
