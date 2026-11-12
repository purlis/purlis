// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Links in this pane…'s key** (#6220, D5): ⇧⌘L, and Ctrl+Shift+L off a Mac. A leaf, as
 * `keys/findKeys.ts` is: the read-only terminal reads it without the registry, which reaches
 * the person's state (the theme specimen holds that, `specimen/Specimen.test.tsx`).
 */

type Held = Pick<KeyboardEvent, "key" | "metaKey" | "ctrlKey" | "altKey" | "shiftKey">;

/** Whether `e` asks for *Links in this pane…*. */
export function opensTheLinksList(e: Held, mac: boolean): boolean {
  return (
    e.key.toLowerCase() === "l" &&
    e.shiftKey &&
    !e.altKey &&
    (mac ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey)
  );
}

/** How the key is spelled: the prototype's D5, in the Mac's own order (⇧ before ⌘). */
export const linksListKeySaid = (mac: boolean) => (mac ? "⇧⌘L" : "Ctrl+Shift+L");
