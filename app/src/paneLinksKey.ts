// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Links in this pane…'s keys** (#6220, D5): ⇧⌘L opens it (Ctrl+Shift+L off a Mac), and ⌘C
 * on a row copies its target (Ctrl+C). The registry declares both (`pane.links`,
 * `links.copy`) with these matchers. A leaf, as
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

/**
 * **Whether `e` copies the link a list's row is on** (D5, D6): ⌘C, Ctrl+C off a Mac, with
 * Shift and Alt up, so ⌘⇧C and ⌥⌘C are left to what else has them.
 */
export function copiesALink(e: Held, mac: boolean): boolean {
  return (
    e.key.toLowerCase() === "c" &&
    !e.shiftKey &&
    !e.altKey &&
    (mac ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey)
  );
}

/** How the copy key is spelled. */
export const linkCopyKeySaid = (mac: boolean) => (mac ? "⌘C" : "Ctrl+C");

/** How the key is spelled: the prototype's D5, in the Mac's own order (⇧ before ⌘). */
export const linksListKeySaid = (mac: boolean) => (mac ? "⇧⌘L" : "Ctrl+Shift+L");
