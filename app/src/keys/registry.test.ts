// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import {
  conflicts,
  GROUPS,
  keySaid,
  keystroke,
  overlaps,
  pressed,
  rowKey,
  SCOPES,
  SHORTCUTS,
  shortcut,
  standsBackFor,
  type Platform,
  type Shortcut,
} from "./registry";
import { switcherKeySaid } from "../switcherKey";
import { pinKeySaid, workspacesKeySaid } from "../workspaceSwitch";
import { sideKeysSaid } from "../sideKeys";
import { taskKeySaid } from "../taskKeys";
import { shellKeySaid } from "../shellKey";
import { CHAT_KEYBOARD, RENAMES_ON_F2 } from "../actions";

const PLATFORMS: readonly Platform[] = ["mac", "other"];
const keyed = SHORTCUTS.filter((one) => one.kind === "key");

afterEach(() => {
  document.body.replaceChildren();
});

/** An element in the page, under `wrap` when given, for a keystroke to land on. */
function landOn(attributes: Record<string, string> = {}, wrap?: Record<string, string>) {
  const at = document.createElement("textarea");
  for (const [name, value] of Object.entries(attributes)) at.setAttribute(name, value);
  if (wrap === undefined) {
    document.body.append(at);
    return at;
  }
  const holder = document.createElement("div");
  for (const [name, value] of Object.entries(wrap)) holder.setAttribute(name, value);
  holder.append(at);
  document.body.append(holder);
  return at;
}

describe("the key registry (N24)", () => {
  it("holds each act once, in a group of the sheet and a place of the window", () => {
    const ids = SHORTCUTS.map((one) => one.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const one of SHORTCUTS) {
      expect(GROUPS).toContain(one.group);
      expect(SCOPES).toContain(one.where);
      expect(one.does).toMatch(/^[A-Z]/);
    }
  });

  it("declares ⌘P, ⌘1–⌘9, Esc and ⏎ paths", () => {
    expect(keySaid("finder.open", true)).toBe("⌘P");
    expect(keySaid("finder.back", true)).toBe("⌘P ↩");
    expect(shortcut("workspace.pin").chords.mac).toHaveLength(9);
    expect(keySaid("workspace.pin", true)).toBe("⌘1–⌘9");
    for (const id of ["palette.close", "dialog.close", "menu.close"])
      expect(shortcut(id).chords.mac).toEqual([{ key: "Escape" }]);
    for (const id of ["palette.run", "dialog.default"])
      expect(shortcut(id).chords.other).toEqual([{ key: "Enter" }]);
  });

  it("puts Ctrl in ⌘'s place off a Mac, but where a chat would lose the chord", () => {
    // Each of these adds Shift or Alt off a Mac, for the reason beside it in its own module: a
    // Ctrl letter is a byte to a terminal (Ctrl+P, Ctrl+F), Ctrl with Alt is AltGr, or the
    // operator's ruling spells it so (Ctrl+Alt+Shift+P). A tab's next and previous chat are its
    // menu's Down and Up on a Mac, and the brackets elsewhere, where Ctrl+Alt+arrows are the
    // desktop's (`taskKeys.ts`).
    const apart = new Set([
      "tasks.next",
      "tasks.previous",
      "finder.open",
      "workspaces.open",
      "chat.find",
      "finder.copyPath",
      "finder.reveal",
      "app.quit",
    ]);
    for (const one of SHORTCUTS) {
      if (apart.has(one.id)) continue;
      for (const chord of one.chords.mac.filter((mac) => mac.meta === true)) {
        const other = one.chords.other.find(
          (theirs) => theirs.key.toLowerCase() === chord.key.toLowerCase() && theirs.ctrl === true,
        );
        if (one.id === "app.hide") continue; // A Mac's alone.
        expect(other, `${one.id} off a Mac`).toBeDefined();
        expect(other?.meta, one.id).toBeUndefined();
        expect(other?.shift, one.id).toBe(chord.shift);
        expect(other?.alt, one.id).toBe(chord.alt);
      }
    }
  });

  it.each(PLATFORMS)("recognises each key's own chords on a %s keyboard", (platform) => {
    const mac = platform === "mac";
    for (const one of keyed)
      for (const chord of one.chords[platform])
        expect(one.matches?.(keystroke(chord), mac), `${one.id} ${JSON.stringify(chord)}`).toBe(
          true,
        );
  });

  it("spells each key as the rows, tooltips and docs always have", () => {
    for (const mac of [true, false]) {
      expect(keySaid("finder.open", mac)).toBe(switcherKeySaid(mac));
      expect(keySaid("workspaces.open", mac)).toBe(workspacesKeySaid(mac));
      expect(keySaid("workspace.pin", mac)).toBe(`${pinKeySaid(1, mac)}–${pinKeySaid(9, mac)}`);
      expect(keySaid("shell.new", mac)).toBe(shellKeySaid(mac));
      expect(keySaid("tasks.next", mac)).toBe(taskKeySaid("next", mac));
      const side = sideKeysSaid(mac);
      expect(keySaid("region.navigation", mac)).toBe(side.navigation);
      expect(keySaid("region.aside", mac)).toBe(side.aside);
      expect(keySaid("view.changes", mac)).toBe(side.changes);
      expect(keySaid("region.next", mac)).toBe(side.next);
    }
  });
});

describe("conflicts (two acts on one key in one place)", () => {
  it("there are none", () => {
    expect(conflicts()).toEqual([]);
  });

  it.each(PLATFORMS)(
    "no keystroke of one act is another's where both apply, on a %s keyboard",
    (platform) => {
      const mac = platform === "mac";
      const found: string[] = [];
      for (const one of SHORTCUTS)
        for (const chord of one.chords[platform]) {
          const event = keystroke(chord);
          for (const other of keyed) {
            if (other === one || !overlaps(one.where, other.where)) continue;
            if (other.matches?.(event, mac) !== true) continue;
            if (
              standsBackFor(one, other, platform, chord) ||
              standsBackFor(other, one, platform, chord)
            )
              continue;
            found.push(`${one.id} is ${other.id}'s too`);
          }
        }
      expect(found).toEqual([]);
    },
  );

  /** An act for a fixture, everywhere in the window, on ⌘G and Ctrl+G. */
  const act = (id: string, more: Partial<Shortcut> = {}): Shortcut => ({
    id,
    does: "A fixture",
    group: "Window",
    where: "window",
    kind: "key",
    chords: { mac: [{ key: "g", meta: true }], other: [{ key: "g", ctrl: true }] },
    said: () => "⌘G",
    yields: [],
    ...more,
  });

  it("finds two acts on one chord in one place, on each keyboard", () => {
    expect(conflicts([act("one"), act("two")])).toEqual([
      { chord: "meta+g", platform: "mac", ids: ["one", "two"] },
      { chord: "ctrl+g", platform: "other", ids: ["one", "two"] },
    ]);
  });

  it("finds a key of the window's on a place's key", () => {
    expect(conflicts([act("window"), act("chat", { where: "terminal" })])).toHaveLength(2);
  });

  it("lets two places that never meet have one key", () => {
    expect(
      conflicts([act("in a chat", { where: "terminal" }), act("in a file", { where: "editor" })]),
    ).toEqual([]);
  });

  it("lets one act stand back for the other, on the keyboard its rule names", () => {
    const window = act("window", { yields: ["terminal-off-mac"] });
    expect(conflicts([window, act("chat", { where: "terminal" })])).toEqual([
      { chord: "meta+g", platform: "mac", ids: ["window", "chat"] },
    ]);
  });

  it("reads a letter whatever its case", () => {
    const shifted = act("shifted", { chords: { mac: [{ key: "G", meta: true }], other: [] } });
    expect(conflicts([act("one"), shifted])).toHaveLength(1);
  });
});

describe("the context rules", () => {
  it("holds a key back while a modal dialog is up", () => {
    const dialog = document.createElement("div");
    dialog.dataset.scope = "dialog";
    dialog.dataset.part = "root";
    document.body.append(dialog);
    const pin = keystroke({ key: "3", meta: true });
    expect(pressed(pin, ["workspace.pin"], { mac: true })).toBeUndefined();
    // An alert dialog is modal too.
    dialog.remove();
    const alert = document.createElement("div");
    alert.setAttribute("role", "alertdialog");
    document.body.append(alert);
    expect(pressed(pin, ["workspace.pin"], { mac: true })).toBeUndefined();
    alert.remove();
    expect(pressed(pin, ["workspace.pin"], { mac: true })?.id).toBe("workspace.pin");
  });

  it("lets a modal's own keys through while it is the one up", () => {
    const dialog = document.createElement("div");
    dialog.dataset.scope = "dialog";
    dialog.dataset.part = "root";
    document.body.append(dialog);
    const k = keystroke({ key: "k", meta: true });
    expect(pressed(k, ["palette.open"], { mac: true })).toBeUndefined();
    expect(pressed(k, ["palette.open"], { mac: true, ownModal: true })?.id).toBe("palette.open");
  });

  it("holds the title bar's keys back while the palette is up", () => {
    const popover = keystroke({ key: "p", code: "KeyP", meta: true, alt: true });
    expect(pressed(popover, ["workspaces.open"], { mac: true })?.id).toBe("workspaces.open");
    expect(pressed(popover, ["workspaces.open"], { mac: true, palette: true })).toBeUndefined();
  });

  it("leaves a chat its Ctrl chords off a Mac, and takes ⌘ chords from it on one", () => {
    const terminal = landOn({}, { [CHAT_KEYBOARD]: "" });
    const off = keystroke({ key: "b", ctrl: true }, terminal);
    expect(pressed(off, ["region.navigation"], { mac: false })).toBeUndefined();
    const on = keystroke({ key: "b", meta: true }, terminal);
    expect(pressed(on, ["region.navigation"], { mac: true })?.id).toBe("region.navigation");
  });

  it("takes the workspace popover's key from a chat on purpose", () => {
    const terminal = landOn({}, { [CHAT_KEYBOARD]: "" });
    const key = keystroke({ key: "P", code: "KeyP", ctrl: true, alt: true, shift: true }, terminal);
    expect(pressed(key, ["workspaces.open"], { mac: false })?.id).toBe("workspaces.open");
  });

  it("leaves a chat Ctrl+K on either keyboard, and F2 to a tab that renames on it", () => {
    const terminal = landOn({}, { [CHAT_KEYBOARD]: "" });
    expect(
      pressed(keystroke({ key: "k", ctrl: true }, terminal), ["palette.open"], { mac: true }),
    ).toBeUndefined();
    expect(
      pressed(keystroke({ key: "k", meta: true }, terminal), ["palette.open"], { mac: true })?.id,
    ).toBe("palette.open");
    const tab = landOn({ [RENAMES_ON_F2]: "" });
    expect(pressed(keystroke({ key: "F2" }, tab), ["palette.open"])).toBeUndefined();
  });

  it("leaves an editor its own ⌘/", () => {
    const editor = landOn({}, { class: "cm-editor" });
    expect(
      pressed(keystroke({ key: "/", meta: true }, editor), ["shortcuts.show"], { mac: true }),
    ).toBeUndefined();
    expect(
      pressed(keystroke({ key: "/", meta: true }), ["shortcuts.show"], { mac: true })?.id,
    ).toBe("shortcuts.show");
  });

  it("leaves a field a task key whose row cannot run", () => {
    const field = landOn();
    const key = keystroke({ key: "J", ctrl: true, shift: true }, field);
    expect(pressed(key, ["tasks.menu"], { mac: false, runnable: false })).toBeUndefined();
    expect(pressed(key, ["tasks.menu"], { mac: false, runnable: true })?.id).toBe("tasks.menu");
  });

  it("answers the first of a handler's acts that is this key, and none where it yields", () => {
    const key = keystroke({ key: "j", meta: true });
    expect(pressed(key, ["region.navigation", "region.panel"], { mac: true })?.id).toBe(
      "region.panel",
    );
    expect(pressed(key, ["region.navigation"], { mac: true })).toBeUndefined();
  });
});

describe("the key a row shows (N25)", () => {
  it("is the registry's, by the row's id or its prefix", () => {
    expect(rowKey("shell.new", true)).toBe("⌘⇧T");
    expect(rowKey("shell.new", false)).toBe("Ctrl+Shift+T");
    expect(rowKey("tab.close:7", true)).toBe("⌫");
    expect(rowKey("tab.rename:7", false)).toBe("F2");
    expect(rowKey("view.toggle:navigation", true)).toBe("⌘B");
    expect(rowKey("shortcuts.show", true)).toBe("⌘/");
    expect(rowKey("focus.toggle", false)).toBe("Ctrl+Shift+Enter");
  });

  it("is nothing for a row with no key", () => {
    expect(rowKey("extensions.show", true)).toBeUndefined();
    expect(rowKey("tab.closed", true)).toBeUndefined();
  });
});
