// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  drivenBy,
  handsBack,
  isDriven,
  keyInput,
  modifiersOf,
  mouseInput,
  pagePoint,
  sendToDriven,
  wheelInput,
} from "./browserDriving";

const none = { altKey: false, ctrlKey: false, metaKey: false, shiftKey: false };

function chord(key: string, held: Partial<typeof none>): KeyboardEvent {
  return new KeyboardEvent("keydown", { key, ...none, ...held });
}

describe("driving a browser from its view", () => {
  it("lands a point of the view where the page is drawn, scaled to fit and centred", () => {
    // A 1280×800 page drawn in a 640×600 box: half size, 100 px of letterbox above and below.
    const box = { left: 10, top: 20, width: 640, height: 600 };
    const page = { width: 1280, height: 800 };

    expect(pagePoint({ x: 10, y: 120 }, box, page)).toEqual({ x: 0, y: 0 });
    expect(pagePoint({ x: 330, y: 320 }, box, page)).toEqual({ x: 640, y: 400 });
    expect(pagePoint({ x: 330, y: 60 }, box, page)).toBeUndefined();
  });

  it("names the keys held as the bits the session host reads", () => {
    expect(modifiersOf(none)).toBe(0);
    expect(modifiersOf({ ...none, altKey: true, shiftKey: true })).toBe(9);
    expect(modifiersOf({ ...none, ctrlKey: true, metaKey: true })).toBe(6);
  });

  it("hands back on the one chord, and on no other", () => {
    expect(handsBack(chord("Enter", { metaKey: true, shiftKey: true }), true)).toBe(true);
    expect(handsBack(chord("Enter", { ctrlKey: true, shiftKey: true }), false)).toBe(true);
    expect(handsBack(chord("Enter", { shiftKey: true }), true)).toBe(false);
    expect(handsBack(chord("Enter", { ctrlKey: true, shiftKey: true }), true)).toBe(false);
    expect(handsBack(chord("a", { metaKey: true, shiftKey: true }), true)).toBe(false);
  });

  it("aims each event at what the frame the person saw showed", () => {
    const at = { x: 12, y: 34 };
    const click = { button: 0, detail: 2, ...none };

    expect(mouseInput("p1:5", "pressed", at, click)).toEqual({
      kind: "mouse",
      aim: "p1:5",
      action: "pressed",
      x: 12,
      y: 34,
      button: "left",
      clicks: 2,
      modifiers: 0,
      delta_x: 0,
      delta_y: 0,
    });
    expect(mouseInput("p1:5", "moved", at, click)).toMatchObject({ button: "none", clicks: 0 });
    expect(wheelInput("p1:5", at, { deltaX: 0, deltaY: 99.6, ...none })).toMatchObject({
      action: "wheel",
      delta_y: 100,
    });
    expect(
      keyInput("p2:1", true, { key: "A", code: "KeyA", keyCode: 65, ...none, shiftKey: true }),
    ).toEqual({
      kind: "key",
      aim: "p2:1",
      action: "down",
      key: "A",
      physical: "KeyA",
      key_code: 65,
      modifiers: 8,
    });
  });

  it("passes a key purlis claimed to the browser the person drives, and to none other", () => {
    const sent: string[] = [];
    const stop = drivenBy("B1", (key) => sent.push(key));

    expect(isDriven("B1")).toBe(true);
    expect(sendToDriven("B1", "F2")).toBe(true);
    expect(sendToDriven("B2", "F2")).toBe(false);
    stop();
    expect(isDriven("B1")).toBe(false);
    expect(sendToDriven("B1", "F2")).toBe(false);
    expect(sent).toEqual(["F2"]);
  });
});
