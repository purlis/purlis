// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { paneStepOf, paneStepSaid, tabStepOf, tabStepSaid } from "./moveKeys";

const key = (init: KeyboardEventInit) => new KeyboardEvent("keydown", init);

describe("the pane keys (#4767)", () => {
  it("are ⌘⌥ and an arrow on a Mac", () => {
    const held = { metaKey: true, altKey: true };
    expect(paneStepOf(key({ key: "ArrowLeft", ...held }), true)).toBe("left");
    expect(paneStepOf(key({ key: "ArrowRight", ...held }), true)).toBe("right");
    expect(paneStepOf(key({ key: "ArrowUp", ...held }), true)).toBe("up");
    expect(paneStepOf(key({ key: "ArrowDown", ...held }), true)).toBe("down");
    // Option and an arrow alone is the terminal's word movement, and ⌘ and an arrow a field's.
    expect(paneStepOf(key({ key: "ArrowLeft", altKey: true }), true)).toBeUndefined();
    expect(paneStepOf(key({ key: "ArrowLeft", metaKey: true }), true)).toBeUndefined();
  });

  it("are Alt and an arrow elsewhere, never with Ctrl, which is the desktop's", () => {
    expect(paneStepOf(key({ key: "ArrowDown", altKey: true }), false)).toBe("down");
    expect(
      paneStepOf(key({ key: "ArrowDown", altKey: true, ctrlKey: true }), false),
    ).toBeUndefined();
    expect(
      paneStepOf(key({ key: "ArrowDown", altKey: true, shiftKey: true }), false),
    ).toBeUndefined();
  });

  it("are spelled as the ruling spells them", () => {
    expect(paneStepSaid(true)).toBe("⌘⌥←↑→↓");
    expect(paneStepSaid(false)).toBe("Alt+←↑→↓");
  });
});

describe("the next and previous tab (#4767)", () => {
  it("are ⌘⇧] and ⌘⇧[ on a Mac, and Ctrl+Shift with them elsewhere", () => {
    expect(
      tabStepOf(key({ key: "}", code: "BracketRight", metaKey: true, shiftKey: true }), true),
    ).toBe("next");
    expect(
      tabStepOf(key({ key: "{", code: "BracketLeft", metaKey: true, shiftKey: true }), true),
    ).toBe("previous");
    const held = { ctrlKey: true, shiftKey: true };
    expect(tabStepOf(key({ key: "}", code: "BracketRight", ...held }), false)).toBe("next");
    expect(tabStepOf(key({ key: "{", code: "BracketLeft", ...held }), false)).toBe("previous");
    expect(tabStepSaid("next", true)).toBe("⌘⇧]");
    expect(tabStepSaid("previous", false)).toBe("Ctrl+Shift+[");
  });

  it("leaves ⌘[ and ⌘] to a view's Back and Forward, and plain Ctrl+[ to the shell", () => {
    expect(tabStepOf(key({ key: "[", code: "BracketLeft", metaKey: true }), true)).toBeUndefined();
    expect(tabStepOf(key({ key: "[", code: "BracketLeft", ctrlKey: true }), false)).toBeUndefined();
    // On a Mac every Ctrl chord is the terminal's.
    expect(
      tabStepOf(key({ key: "{", code: "BracketLeft", ctrlKey: true, shiftKey: true }), true),
    ).toBeUndefined();
  });

  it("takes the key in a bracket's place where the layout's brackets need AltGr", () => {
    const held = { ctrlKey: true, shiftKey: true };
    expect(tabStepOf(key({ key: "*", code: "BracketRight", ...held }), false)).toBe("next");
    expect(tabStepOf(key({ key: "¨", code: "BracketLeft", ...held }), false)).toBe("previous");
    // A letter there is that layout's letter, and is not taken.
    expect(tabStepOf(key({ key: "Ü", code: "BracketLeft", ...held }), false)).toBeUndefined();
  });

  it("does not fire with the text size's or the shortcuts' key on Dvorak", () => {
    const held = { ctrlKey: true, shiftKey: true };
    expect(tabStepOf(key({ key: "+", code: "BracketRight", ...held }), false)).toBeUndefined();
    expect(tabStepOf(key({ key: "?", code: "BracketLeft", ...held }), false)).toBeUndefined();
    // Dvorak's own ] is elsewhere, and is the key by what it types.
    expect(tabStepOf(key({ key: "}", code: "Equal", ...held }), false)).toBe("next");
  });
});
