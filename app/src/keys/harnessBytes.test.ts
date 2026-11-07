// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { SHORTCUTS, type Chord, type Platform } from "./registry";

/** What xterm.js's `evaluateKeyboardEvent` is handed, and the part of its answer read here. */
type XtermKey = {
  type: string;
  altKey: boolean;
  ctrlKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
  keyCode: number;
  key: string;
  code: string;
};
type Evaluate = (
  ev: XtermKey,
  applicationCursorMode: boolean,
  isMac: boolean,
  macOptionIsMeta: boolean,
) => { key: string | undefined };

/**
 * The pinned xterm.js's own key mapping, from the sources it ships beside its bundle. Loaded
 * by the test runner only, which resolves their `common/…` imports by the unit project's alias
 * in `vite.config.ts`; the type check never reads them.
 */
const KEYBOARD = "../../node_modules/@xterm/xterm/src/common/input/Keyboard.ts";
const { evaluateKeyboardEvent } = (await import(/* @vite-ignore */ KEYBOARD)) as {
  evaluateKeyboardEvent: Evaluate;
};

/**
 * **None of the window's keys for panes, tabs and a chat's tasks is a byte a harness receives**
 * (#4767, ruled on #4699). Each is claimed from a chat's terminal because xterm.js would send
 * nothing for it, and this reads that answer from the pinned xterm.js's own key-to-bytes mapping
 * (`evaluateKeyboardEvent`), not from what we remember of it. If a new xterm.js starts sending
 * bytes for one (the kitty keyboard protocol would), this fails and the chord needs a review.
 *
 * A chord the registry leaves to the terminal (Alt and an arrow off a Mac) is held to the
 * opposite: it IS a byte, and that is why it is left.
 */

/** The window's keys this ruling claims. Their chords are read from the registry, so a chord
 *  added to one of them is covered; a new key is added here. */
const CLAIMED = ["pane.focus", "tab.next", "tab.previous", "tasks.menu", "pane.back", "pane.move"];

/** The key codes xterm.js reads, for the keys these chords use. */
const KEY_CODES: Record<string, number> = {
  ArrowLeft: 37,
  ArrowUp: 38,
  ArrowRight: 39,
  ArrowDown: 40,
  "[": 219,
  "{": 219,
  "]": 221,
  "}": 221,
  j: 74,
  h: 72,
  // Placement mode's ⌘⇧M, Ctrl+Shift+M off a Mac (#4764).
  m: 77,
};

/** What xterm.js would send for `chord`, typed on a `platform` keyboard; nothing when it sends
 *  nothing. Both cursor modes, since a full-screen program switches to the application one. */
function bytesOf(chord: Chord, platform: Platform): string[] {
  const keyCode = KEY_CODES[chord.key.length === 1 ? chord.key.toLowerCase() : chord.key];
  if (keyCode === undefined) throw new Error(`no key code for ${chord.key}`);
  const typed =
    chord.key.length === 1 && chord.shift === true ? chord.key.toUpperCase() : chord.key;
  const sent: string[] = [];
  for (const application of [false, true]) {
    const result = evaluateKeyboardEvent(
      {
        type: "keydown",
        altKey: chord.alt === true,
        ctrlKey: chord.ctrl === true,
        shiftKey: chord.shift === true,
        metaKey: chord.meta === true,
        keyCode,
        key: typed,
        code: chord.code ?? "",
      },
      application,
      platform === "mac",
      false,
    );
    if (result.key !== undefined) sent.push(result.key);
  }
  return sent;
}

describe("the keys for panes, tabs and a chat's tasks are no bytes to a harness (xterm.js)", () => {
  const cases = SHORTCUTS.filter((one) => CLAIMED.includes(one.id)).flatMap((one) =>
    (["mac", "other"] as const).flatMap((platform) =>
      one.chords[platform].map((chord) => ({ one, platform, chord })),
    ),
  );

  it("covers every chord of each claimed key", () => {
    expect(new Set(cases.map((each) => each.one.id))).toEqual(new Set(CLAIMED));
  });

  it.each(cases)("$one.id $chord.key on a $platform keyboard", ({ one, platform, chord }) => {
    const leftToTheChat =
      one.yields.includes("terminal") ||
      (platform === "other" && one.yields.includes("terminal-off-mac"));
    if (leftToTheChat) expect(bytesOf(chord, platform)).not.toEqual([]);
    else expect(bytesOf(chord, platform)).toEqual([]);
  });
});
