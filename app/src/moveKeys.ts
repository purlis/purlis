// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Toward } from "./panePlaces";
import { sizeKey } from "./textSize";

/**
 * **The keys that move the keyboard between panes and tabs** (#4767, ruled on #4699):
 *
 * | | On a Mac | Everywhere else |
 * |---|---|---|
 * | The pane in that direction | ⌘⌥ ← ↑ → ↓ | Alt+← ↑ → ↓ (not while a chat has the keyboard) |
 * | The next / previous tab | ⌘⇧] / ⌘⇧[ | Ctrl+Shift+] / Ctrl+Shift+[ |
 *
 * **Why these.** ⌘⌥ and an arrow moves between panes in iTerm2, and Alt and an arrow in Windows
 * Terminal; ⌘⇧] and ⌘⇧[ are next and previous tab in every tabbed program on a Mac, and were
 * kept free for this. Off a Mac Ctrl+Alt with an arrow is the desktop's, and Ctrl+PageUp and
 * Ctrl+PageDown a terminal's.
 *
 * **What they take from a chat**, by the rule `docs/ui-primitives.md` holds every claimed key to,
 * which `keys/harnessBytes.test.ts` reads from the pinned xterm.js's own key mapping: an arrow with
 * ⌘ sends nothing, and Ctrl+Shift with a bracket sends nothing, so those are the window's even
 * while a chat has the keyboard. **Alt and an arrow is a byte** (word movement, `ESC [1;3D`), so
 * off a Mac it is left to the chat while a terminal has the keyboard: there Ctrl+Tab leaves the
 * terminal first. It is a view's Back and Forward off a Mac too (`keys/viewKeys.ts`), and a view
 * keeps them.
 */

/** Which way the next or previous tab is. */
export type TabStep = "next" | "previous";

/** The pane this keystroke moves the keyboard toward, if it is one of these keys. */
export function paneStepOf(e: KeyboardEvent, mac: boolean): Toward | undefined {
  if (e.ctrlKey || e.shiftKey || !e.altKey) return undefined;
  if (e.metaKey !== mac) return undefined;
  return ARROWS[e.key];
}

const ARROWS: Readonly<Record<string, Toward>> = {
  ArrowLeft: "left",
  ArrowRight: "right",
  ArrowUp: "up",
  ArrowDown: "down",
};

/**
 * Whether this keystroke is the next or the previous tab.
 *
 * **By what the key types, not by where it is**, as `taskKeys.ts` reads its letters: only where
 * the layout has no `[` or `]` without AltGr (German, Nordic, French) is the key in their place
 * taken, and then never when it types a letter or a digit, and never when it is another of the
 * window's own keys: on Dvorak that key types `=` and `+`, the text size's, and `/` and `?`, the
 * shortcuts sheet's.
 */
export function tabStepOf(e: KeyboardEvent, mac: boolean): TabStep | undefined {
  if (!heldWithShift(e, mac)) return undefined;
  if (e.key === "]" || e.key === "}") return "next";
  if (e.key === "[" || e.key === "{") return "previous";
  if (e.code !== "BracketRight" && e.code !== "BracketLeft") return undefined;
  if (sizeKey(e, mac) !== undefined || /^[\p{L}\p{N}/?]$/u.test(e.key)) return undefined;
  return e.code === "BracketRight" ? "next" : "previous";
}

/** How the rows and the sheet spell the pane keys, on this platform. */
export function paneStepSaid(mac: boolean): string {
  return mac ? "⌘⌥←↑→↓" : "Alt+←↑→↓";
}

/** How the rows and the sheet spell the next or the previous tab's key. */
export function tabStepSaid(step: TabStep, mac: boolean): string {
  return withShiftSaid(step === "next" ? "]" : "[", mac);
}

/**
 * Whether ⌘⇧ is held on a Mac, or Ctrl+Shift elsewhere, and nothing else: the modifiers of the
 * tab keys here and of the focused chat's keys (`taskKeys.ts`). A Mac's Ctrl chords stay a
 * terminal's, and Alt with them is AltGr off a Mac.
 */
export function heldWithShift(e: KeyboardEvent, mac: boolean): boolean {
  if (!e.shiftKey || e.altKey) return false;
  return mac ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
}

/** How a ⌘⇧ chord is spelled on this platform: `⌘⇧J`, or `Ctrl+Shift+J`. */
export function withShiftSaid(key: string, mac: boolean): string {
  return mac ? `⌘⇧${key}` : `Ctrl+Shift+${key}`;
}
