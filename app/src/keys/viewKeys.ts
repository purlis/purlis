// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A view tab's back and forward keys** (#3667): ⌘[ and ⌘] on a Mac, Alt+← and Alt+→ elsewhere.
 *
 * **Why these.** ⌘[ and ⌘] are Back and Forward in every Mac program with a history (Finder,
 * Safari, Xcode, VS Code), and nothing of the window's had them: ⌘⇧[ and ⌘⇧] stay free for the
 * strip's next and previous tab (`taskKeys.ts`). Off a Mac Ctrl does not take ⌘'s place here:
 * Ctrl+[ is Escape to a terminal and Ctrl+[ Ctrl+] are an editor's indent, while Alt+← and Alt+→
 * are Back and Forward in every browser and in VS Code on Windows and Linux. A bracket is read by
 * what the key types, and where a layout has no bracket without AltGr, by the key in its place.
 */
import { sizeKey } from "../textSize";

export type ViewHistoryKey = "back" | "forward";

export function viewHistoryKeyOf(e: KeyboardEvent, mac: boolean): ViewHistoryKey | undefined {
  if (mac) {
    if (!e.metaKey || e.ctrlKey || e.altKey || e.shiftKey) return undefined;
    if (e.key === "[") return "back";
    if (e.key === "]") return "forward";
    // The key in a bracket's place, never when it types a letter or a digit, and never when it
    // is another of the window's keys: on Dvorak it types `=` and `/`, the text size's and the
    // shortcuts' (as `taskKeys.ts` reads it).
    if (/^[\p{L}\p{N}]$/u.test(e.key) || e.key === "/" || sizeKey(e, mac) !== undefined)
      return undefined;
    if (e.code === "BracketLeft") return "back";
    if (e.code === "BracketRight") return "forward";
    return undefined;
  }
  if (!e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return undefined;
  if (e.key === "ArrowLeft") return "back";
  if (e.key === "ArrowRight") return "forward";
  return undefined;
}
