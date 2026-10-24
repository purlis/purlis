// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { layoutPref } from "./layoutPref";

/**
 * **Whether a chat's terminal is in xterm's screen-reader mode** (DS-6, #629, D-629-2).
 *
 * Off until the person turns it on, and then on in every terminal at once, the ones up now
 * and every one drawn after. Off by default because the mode costs every pane something
 * nobody else needs: xterm keeps a DOM row for each row of the screen and rewrites them as
 * output arrives, on the path a tab switch and a burst of output are measured on.
 *
 * **Kept in the layout file** (`layoutPref.ts`, `regions.ts`, D-629-5): how one person needs
 * their window on this machine, beside their editor and how the Chats list is drawn, under
 * `screenReader`, written only while it is on. It is the window's own document, not the
 * project's, so nothing is committed and no other clone is changed. *Use the default layout*
 * leaves it as it is (D-629-6): a person who needs a screen reader would otherwise lose the
 * one way the terminal reads to them by tidying their panes.
 */

/** Screen-reader mode as a layout document holds it, and what had to be put right. */
export function loadScreenReaderMode(raw: unknown): { on: boolean; said: string[] } {
  const held =
    raw !== null && typeof raw === "object" && !Array.isArray(raw)
      ? (raw as { screenReader?: unknown }).screenReader
      : undefined;
  if (held === undefined || typeof held === "boolean") return { on: held === true, said: [] };
  return {
    on: false,
    said: [`"screenReader" ${JSON.stringify(held)} is not true or false, so it is off`],
  };
}

/** The store (`layoutPref.ts`), under `screenReader`. */
export const SCREEN_READER = layoutPref<boolean, "screenReader">({
  key: "screenReader",
  fallback: false,
  load: (raw) => {
    const { on, said } = loadScreenReaderMode(raw);
    return { value: on, said };
  },
  same: (one, other) => one === other,
  written: (on) => (on ? true : undefined),
  remedy: (where) =>
    `fix ${where}, or turn screen-reader mode on or off from the command palette, which rewrites it`,
});

/** Whether a chat's terminal is in screen-reader mode now. */
export function screenReaderMode(): boolean {
  return SCREEN_READER.value();
}

/** Turns screen-reader mode on or off in every chat's terminal; the layout file keeps it. */
export function setScreenReaderMode(on: boolean): void {
  SCREEN_READER.set(on);
}

/** Calls `heard` each time screen-reader mode is turned on or off; answers how to stop. */
export function onScreenReaderMode(heard: (on: boolean) => void): () => void {
  return SCREEN_READER.on(heard);
}

/** Screen-reader mode, drawn again whenever it is turned on or off: for what a render reads of
 *  it, such as the palette's row whose words say what pressing it does now. */
export function useScreenReaderMode(): boolean {
  return SCREEN_READER.use();
}

/** Forgets what this launch chose and read, as a new launch would. For tests. */
export function forgetScreenReaderMode(): void {
  SCREEN_READER.forget();
}
