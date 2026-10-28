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
  rowAriaKeys,
  rowKey,
  SCOPES,
  SHORTCUTS,
  shortcut,
  spokenKeys,
  standsBackFor,
  type Chord,
  type Platform,
  type Shortcut,
} from "./registry";
import { switcherKeySaid } from "../switcherKey";
import { pinKeySaid, workspacesKeySaid } from "../workspaceSwitch";
import { sideKeysSaid } from "../sideKeys";
import { taskKeySaid } from "../taskKeys";
import { shellKeySaid } from "../shellKey";
import { CHAT_KEYBOARD, RENAMES_ON_F2 } from "../actions";
import { EDITOR_HOLDER } from "./contexts";
import { defaultKeymap, historyKeymap } from "@codemirror/commands";
import type { KeyBinding } from "@codemirror/view";

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
      // Ctrl+[ is Escape to a terminal and Ctrl+[ ] an editor's indent; Alt+← → are Back and
      // Forward off a Mac (`viewKeys.ts`).
      "view.back",
      "view.forward",
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
    const editor = landOn({}, { [EDITOR_HOLDER]: "" });
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

/**
 * A CodeMirror key (`Mod-Shift-k`) as a chord on a keyboard: `Mod` is ⌘ on a Mac and Ctrl
 * elsewhere, as CodeMirror reads it.
 */
function editorChord(spec: string, mac: boolean): Chord {
  const parts = spec.split(/-(?!$)/);
  const key = parts.pop() ?? "";
  const chord: Chord = { key: key === "Space" ? " " : key };
  for (const part of parts) {
    if (part === "Mod") chord[mac ? "meta" : "ctrl"] = true;
    else if (part === "Cmd" || part === "Meta") chord.meta = true;
    else if (part === "Ctrl") chord.ctrl = true;
    else if (part === "Alt") chord.alt = true;
    else if (part === "Shift") chord.shift = true;
  }
  return chord;
}

/** Every key the editors' keymaps bind on a keyboard (`editor/TextEditor.tsx` uses these two). */
function editorKeys(mac: boolean): { spec: string; chord: Chord }[] {
  return [...defaultKeymap, ...historyKeymap].flatMap((binding: KeyBinding) => {
    const specs = mac
      ? [binding.mac ?? binding.key]
      : [binding.win ?? binding.key, binding.linux ?? binding.key];
    return [...new Set(specs)].flatMap((spec) => {
      if (spec === undefined) return [];
      const chord = editorChord(spec, mac);
      return binding.shift === undefined || chord.shift === true
        ? [{ spec, chord }]
        : [
            { spec, chord },
            { spec: `Shift-${spec}`, chord: { ...chord, shift: true } },
          ];
    });
  });
}

/**
 * **What the window has always taken from an editor**, each with why it stays: a key the
 * editor's keymap binds that the window answers first. Each is a ticket's to settle, not this
 * test's; a new one fails here.
 */
const TAKEN_FROM_THE_EDITOR = new Set([
  // Escape in an editor inside a dialog: CodeMirror's own Escape only narrows a selection, and
  // takes the key only then; otherwise the dialog closes, which is the way out of it (WCAG 2.1.2).
  "dialog.close on Escape",
]);

describe("the editors' own keys (CodeMirror's keymap)", () => {
  it.each(PLATFORMS)("are the editor's where both apply, on a %s keyboard", (platform) => {
    const mac = platform === "mac";
    const editor = shortcut("file.find");
    const found: string[] = [];
    for (const { spec, chord } of editorKeys(mac)) {
      const event = keystroke(chord);
      for (const one of keyed) {
        if (!overlaps(one.where, "editor") || one.where === "editor") continue;
        if (one.matches?.(event, mac) !== true) continue;
        if (standsBackFor(one, editor, platform, chord)) continue;
        found.push(`${one.id} on ${spec}`);
      }
    }
    expect(found.filter((one) => !TAKEN_FROM_THE_EDITOR.has(one))).toEqual([]);
  });
});

describe("the palette and a code editor (D-2921-25)", () => {
  const editor = () => landOn({}, { [EDITOR_HOLDER]: "" });

  it("leaves the editor its deleteLine and a Mac's deleteToLineEnd", () => {
    expect(
      pressed(keystroke({ key: "K", meta: true, shift: true }, editor()), ["palette.open"], {
        mac: true,
      }),
    ).toBeUndefined();
    expect(
      pressed(keystroke({ key: "k", ctrl: true }, editor()), ["palette.open"], { mac: true }),
    ).toBeUndefined();
    expect(
      pressed(keystroke({ key: "K", ctrl: true, shift: true }, editor()), ["palette.open"], {
        mac: false,
      }),
    ).toBeUndefined();
  });

  it("opens on ⌘K, Ctrl+K off a Mac and F2 in an editor, and on ⌘⇧K anywhere else", () => {
    for (const [chord, mac] of [
      [{ key: "k", meta: true }, true],
      [{ key: "k", ctrl: true }, false],
      [{ key: "F2" }, true],
    ] as const)
      expect(pressed(keystroke(chord, editor()), ["palette.open"], { mac })?.id).toBe(
        "palette.open",
      );
    expect(
      pressed(keystroke({ key: "K", meta: true, shift: true }), ["palette.open"], { mac: true })
        ?.id,
    ).toBe("palette.open");
  });
});

describe("a key in words", () => {
  it("says each symbol and each alternative as a reader hears it", () => {
    expect(spokenKeys("⌘⇧↩", true)).toBe("Command Shift Return");
    expect(spokenKeys("Ctrl+Shift+Enter", false)).toBe("Control Shift Enter");
    expect(spokenKeys("⌘K · F2", true)).toBe("Command K or F2");
    expect(spokenKeys("Ctrl+1–Ctrl+9", false)).toBe("Control 1 to Control 9");
    expect(spokenKeys("⌘/", true)).toBe("Command Slash");
    expect(spokenKeys("Ctrl+Shift+]", false)).toBe("Control Shift Right Bracket");
    expect(spokenKeys("⌥⌘P", true)).toBe("Option Command P");
    expect(spokenKeys("Esc", true)).toBe("Escape");
  });
});

describe("the palette's keys are the window's", () => {
  it("shows a row's key only where it works from anywhere in the window", () => {
    expect(rowKey("tab.close:7", true, true)).toBeUndefined();
    expect(rowKey("tab.rename:7", false, true)).toBeUndefined();
    expect(rowKey("shell.new", true, true)).toBe("⌘⇧T");
    expect(rowAriaKeys("tab.close:7", true, true)).toBeUndefined();
  });
});
