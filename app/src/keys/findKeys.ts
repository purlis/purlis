// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The find bars' keys** (#2921): a chat's (`SessionPane`) and a file's (`LightEditor`), as the
 * key registry reads them (`registry.ts`) without drawing either. Their components hand them on
 * under their old names.
 */
import { onAMac } from "../mac";

/** What a key event says, as much of it as these read: a DOM's or React's. */
type Keyed = Pick<KeyboardEvent, "key" | "altKey" | "shiftKey" | "metaKey" | "ctrlKey">;

/**
 * **Whether this key opens the find bar: `⌘F` on a Mac, `Ctrl+Shift+F` everywhere else.**
 *
 * One modifier per platform, as the text-size keys are (`textSize.sizeKey`), and for the rule
 * `docs/ui-primitives.md` holds every claimed key to. xterm.js 6.0.0 sends nothing for a
 * `⌘`-chord but `⌘A`, so `⌘F` takes nothing from the chat. `Ctrl+F` would: it is `\x06`,
 * readline's forward-char, in the shell every chat starts in — so off a Mac the chord adds
 * `Shift`, as GNOME Terminal and Konsole do for their own find, and xterm sends nothing for a
 * `Ctrl+Shift` letter. A Mac's `Ctrl+F` stays the shell's too.
 */
export function opensFind(event: Keyed, mac: boolean = onAMac()): boolean {
  if (event.key !== "f" && event.key !== "F") return false;
  if (event.altKey) return false;
  return mac
    ? event.metaKey && !event.ctrlKey && !event.shiftKey
    : event.ctrlKey && event.shiftKey && !event.metaKey;
}

/**
 * **Whether this key opens the find bar on a file: `⌘F` on a Mac, `Ctrl+F` everywhere else**
 * (#946). Not a chat's chord (`SessionPane.opensFind`): a file is read, not typed into, so no
 * shell is owed `Ctrl+F`, and it is the key every editor finds with.
 */
export function opensFindInAFile(event: Keyed, mac: boolean = onAMac()): boolean {
  if (event.key !== "f" && event.key !== "F") return false;
  if (event.altKey || event.shiftKey) return false;
  return mac ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
}
