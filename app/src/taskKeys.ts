// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The focused chat's keys** (#4767, ruled on #4699; before it #1487 and #4757):
 *
 * | | On a Mac | Everywhere else |
 * |---|---|---|
 * | The focused chat's tasks | ⌘⇧J | Ctrl+Shift+J |
 * | Back to the chat this pane showed before | ⌘⇧H | Ctrl+Shift+H |
 *
 * A chat is not a tab any more, so the keys that stepped through a tab's chats went with it:
 * stepping would rebuild a terminal on every press and replace each chat it passed over. The
 * sidebar and the palette do that job. ⌘⌥ and the arrows and ⌘⇧] and ⌘⇧[ move between panes and
 * tabs now (`moveKeys.ts`).
 *
 * **Why these.** H is "home", as Finder spells Go › Home, and ⌘⇧H swaps back and forth like
 * `cd -`; J is the free letter beside it. ⌘H alone stays the system's Hide: Shift is part of
 * that chord.
 *
 * **They take nothing from a chat**, by the rule `docs/ui-primitives.md` holds every claimed
 * key to, which `keys/harnessBytes.test.ts` reads from the pinned xterm.js's own key mapping: a
 * `⌘` chord with a letter sends nothing but `⌘A`, and `Ctrl` with a letter is encoded only when
 * Shift is NOT held. So plain Ctrl+J (newline) and Ctrl+H (backspace) still reach the shell. A
 * Mac's `Ctrl` chords stay the terminal's whatever is held with them.
 */
import { heldWithShift, withShiftSaid } from "./moveKeys";

export type TaskKey = "menu" | "back";

/**
 * Which of the two this keystroke is, if any. **By what the key types, not by where it is**: a
 * letter is matched by `key` alone, so a layout that puts another letter on that key fires
 * nothing of this window's twice.
 */
export function taskKeyOf(e: KeyboardEvent, mac: boolean): TaskKey | undefined {
  if (!heldWithShift(e, mac)) return undefined;
  if (e.key === "J" || e.key === "j") return "menu";
  if (e.key === "H" || e.key === "h") return "back";
  return undefined;
}

/** How the rows and the docs spell each key, on this platform. */
export function taskKeySaid(key: TaskKey, mac: boolean): string {
  return withShiftSaid(key === "menu" ? "J" : "H", mac);
}

/** The catalogue's row each key presses. */
export const TASK_KEY_ROW: Readonly<Record<TaskKey, string>> = {
  menu: "tasks.menu",
  back: "pane.back",
};
