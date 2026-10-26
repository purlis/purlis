// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The keys #2921 adds**: the cheat sheet's and focus mode's, chosen by the rule
 * `docs/ui-primitives.md` holds every claimed key to ("Which keys belong to the chat"), read
 * against the pinned `@xterm/xterm` 6.0.0 and every key the registry already holds
 * (`registry.ts`, whose conflict test fails on a clash).
 *
 * | | On a Mac | Everywhere else |
 * | --- | --- | --- |
 * | The keyboard shortcuts (the cheat sheet) | `⌘/` | `Ctrl+/` |
 * | Focus mode: the work alone, and back | `⌘⇧↩` | `Ctrl+Shift+Enter` |
 *
 * **`⌘/` is the shortcuts' key** in the apps an operator already uses (Slack, Google Docs, Gmail's
 * `?` without the modifier). xterm.js sends nothing for a `⌘` chord but `⌘A`, and with `Ctrl` it
 * encodes a letter, space, `3`–`8`, `[`, `\` and `]` only, so `Ctrl+/` is no byte either: it takes
 * nothing from a chat on either platform. **A text editor keeps it**: CodeMirror's default keymap
 * comments a line on `Mod-/`, so the editor that has the keyboard is left it (D-2921-7). `/` is
 * matched by what the key types, so a layout whose `/` needs Shift (German's Shift+7) still has it.
 *
 * **`⌘⇧↩` is focus mode's**, iTerm2's key for its own "the pane alone" (maximize the active
 * pane), and free here: the form family's ⌘↩ is held without Shift (`ui/field/submitKey.ts`), and
 * a chat's Shift+↩ without ⌘. xterm.js sends a bare CR for Enter whatever is held with it but Alt,
 * so a chat that loses this chord loses nothing plain Enter does not send (D-2921-8).
 */

/** Whether this key opens the keyboard shortcuts: ⌘/ on a Mac, Ctrl+/ elsewhere. */
export function opensTheShortcuts(e: KeyboardEvent, mac: boolean): boolean {
  if (e.key !== "/" || e.altKey) return false;
  return mac ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
}

/** How the rows and the docs spell the shortcuts' key, on this platform. */
export function shortcutsKeySaid(mac: boolean): string {
  return mac ? "⌘/" : "Ctrl+/";
}

/** Whether this key turns focus mode on or off: ⌘⇧↩ on a Mac, Ctrl+Shift+Enter elsewhere. */
export function togglesFocus(e: KeyboardEvent, mac: boolean): boolean {
  if (e.key !== "Enter" || !e.shiftKey || e.altKey || e.isComposing) return false;
  return mac ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
}

/** How the rows and the docs spell focus mode's key, on this platform. */
export function focusKeySaid(mac: boolean): string {
  return mac ? "⌘⇧↩" : "Ctrl+Shift+Enter";
}
