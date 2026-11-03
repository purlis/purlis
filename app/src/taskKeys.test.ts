// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { TASK_KEY_ROW, taskKeyOf, taskKeySaid, type TaskKey } from "./taskKeys";

const key = (init: KeyboardEventInit) => new KeyboardEvent("keydown", init);

describe("the focused chat's keys (#4767, ruled on #4699)", () => {
  it("are ⌘⇧J and ⌘⇧H on a Mac", () => {
    const shift = { metaKey: true, shiftKey: true };
    expect(taskKeyOf(key({ key: "J", ...shift }), true)).toBe("menu");
    expect(taskKeyOf(key({ key: "h", ...shift }), true)).toBe("back");
  });

  it("are Ctrl+Shift with J and H everywhere else", () => {
    const held = { ctrlKey: true, shiftKey: true };
    expect(taskKeyOf(key({ key: "j", ...held }), false)).toBe("menu");
    expect(taskKeyOf(key({ key: "H", ...held }), false)).toBe("back");
  });

  it("no longer step through a tab's chats", () => {
    // ⌘⌥ and an arrow is the pane in that direction now, and the brackets the next tab
    // (`moveKeys.ts`).
    expect(taskKeyOf(key({ key: "ArrowDown", metaKey: true, altKey: true }), true)).toBeUndefined();
    expect(
      taskKeyOf(key({ key: "}", code: "BracketRight", ctrlKey: true, shiftKey: true }), false),
    ).toBeUndefined();
  });

  it("matches a letter by what the key types, never by where it is", () => {
    const held = { ctrlKey: true, shiftKey: true };
    expect(taskKeyOf(key({ key: "H", code: "KeyJ", ...held }), false)).toBe("back");
    expect(taskKeyOf(key({ key: "о", code: "KeyJ", ...held }), false)).toBeUndefined();
  });

  it("leaves a chat every chord its terminal turns into bytes", () => {
    // Ctrl with the key and no Shift is the shell's: newline and backspace.
    for (const k of ["j", "h"]) {
      expect(taskKeyOf(key({ key: k, ctrlKey: true }), false)).toBeUndefined();
      // On a Mac every Ctrl chord is the terminal's, whatever is held with it.
      expect(taskKeyOf(key({ key: k, ctrlKey: true, shiftKey: true }), true)).toBeUndefined();
      expect(
        taskKeyOf(key({ key: k, ctrlKey: true, shiftKey: true, altKey: true }), false),
      ).toBeUndefined();
    }
  });

  it("leaves the system's own ⌘H and ⌘J alone: Shift is part of each", () => {
    expect(taskKeyOf(key({ key: "h", metaKey: true }), true)).toBeUndefined();
    expect(taskKeyOf(key({ key: "j", metaKey: true }), true)).toBeUndefined();
  });

  it("says each key as its platform spells it, and names the row it presses", () => {
    const keys: TaskKey[] = ["menu", "back"];
    expect(keys.map((one) => taskKeySaid(one, true))).toEqual(["⌘⇧J", "⌘⇧H"]);
    expect(keys.map((one) => taskKeySaid(one, false))).toEqual(["Ctrl+Shift+J", "Ctrl+Shift+H"]);
    expect(keys.map((one) => TASK_KEY_ROW[one])).toEqual(["tasks.menu", "pane.back"]);
  });
});
