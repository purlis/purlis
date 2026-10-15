// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { contentsOf, type Tabs, type ViewRef } from "./tabs";

/**
 * **A browser, in a view tab** (#2012, spec #2008).
 *
 * A browser runs in the session host and is watched in a view tab of the window: the view
 * `browser`, keyed by the browser's id in the host, drawn by `BrowserView`. Like every view tab
 * it splits like a chat, and the browser lives as long as some tab shows it: when the last one
 * closes, the browser ends with it ({@link browsersLetGo}), since nothing else lists it yet
 * (the Chats list's rows, #2024).
 */

/**
 * **Whether the palette offers New browser.** Not yet: until a browser the person opens can go
 * somewhere (#2176), it could show only a blank page, after the engine's first download. The
 * view, its commands and their tests are all here; #2176 turns this on where it adds the
 * address.
 */
export const BROWSERS_OFFERED: boolean = false;

/** The view a browser is drawn in. */
export const BROWSER_VIEW = "browser";

/** What a browser's tab is called. */
export const BROWSER_TITLE = "Browser";

/** The view tab of browser `program`. */
export function browserView(program: string): ViewRef {
  return { from: null, view: BROWSER_VIEW, key: program };
}

/** Whether `view` is a browser's. */
export function isBrowser(view: ViewRef): boolean {
  return view.from === null && view.view === BROWSER_VIEW && view.key !== "";
}

/** Every browser some tab of `tabs` shows. */
export function browsersShown(tabs: Tabs): Set<string> {
  const shown = new Set<string>();
  for (const id of tabs.order)
    for (const { content } of contentsOf(tabs, id))
      if (content.kind === "view" && isBrowser(content.view)) shown.add(content.view.key);
  return shown;
}

/** The browsers `before` showed and `after` no longer does: theirs was the last tab. */
export function browsersLetGo(before: Tabs, after: Tabs): string[] {
  const still = browsersShown(after);
  return [...browsersShown(before)].filter((program) => !still.has(program));
}
