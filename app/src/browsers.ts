// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { contentsOf, openViewBeside, type Tabs, type ViewRef } from "./tabs";

/**
 * **A browser, in a view tab** (#2012, spec #2008).
 *
 * A browser runs in the session host and is watched in a view tab of the window: the view
 * `browser`, keyed by the browser's id in the host, drawn by `BrowserView`. Like every view tab
 * it splits like a chat. A browser the person opened lives as long as some tab shows it: when
 * the last one closes, the browser ends with it ({@link browsersLetGo}). **A chat's browser**
 * (#2013, #2024) is listed under its chat in the Chats list and drawn live on its chat's tab, so
 * closing a tab that shows it only closes the view: it ends with its chat.
 */

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

/**
 * The browsers `before` showed and `after` no longer does, theirs having been the last tab:
 * each ends. Never one of `endWithTheirChats`: a chat's, which ends with its chat.
 */
export function browsersLetGo(
  before: Tabs,
  after: Tabs,
  endWithTheirChats: ReadonlySet<string> = new Set(),
): string[] {
  const still = browsersShown(after);
  return [...browsersShown(before)].filter(
    (program) => !still.has(program) && !endWithTheirChats.has(program),
  );
}

/** What the tab of chat `chat`'s browser `name` (`b1`, as the chat knows it) is called: by its
 *  name alone where no name of its chat was kept, as a pinned one's may not be (#2022). */
export function chatBrowserTitle(chat: string, name: string): string {
  return chat === "" ? `Browser ${name}` : `Browser ${name} of ${chat}`;
}

/**
 * **A chat's browser, opened beside its chat** (#2024, the setting *Open a chat's browser
 * beside it*): in the chat's tab, to the right of its pane, and nothing else moves. One some
 * tab shows already, or of a chat with no pane, is left as it is.
 */
export function openBrowserBeside(
  tabs: Tabs,
  session: number,
  program: string,
  workspace: string,
): Tabs {
  return openViewBeside(tabs, session, browserView(program), workspace);
}

/** The device a browser runs on, as its chip names it: the window's host, this machine. */
export function thisDevice(onAMac: boolean): string {
  return onAMac ? "this Mac" : "this computer";
}

/**
 * **The device a browser runs on, as its chip names it** (#2050): the runner of `pool` where it
 * runs on one, `pool builds`, so a runner's `localhost` is never taken for this machine's; else
 * this machine.
 */
export function deviceOf(pool: string | null, onAMac: boolean): string {
  return pool === null ? thisDevice(onAMac) : `pool ${pool}`;
}

/**
 * **What a browser's chip says** (#2024, spec #2008): the page's host and the device the browser
 * runs on, `localhost:5173 on this Mac`, drawn by purlis outside the page so a page cannot fake
 * it; `whole` is what it was given, for its tooltip. It is given the page's origin alone
 * (#2300): a path, a query or a title may hold a secret. Before a page, or on a blank one, the
 * device alone.
 */
export function addressChip(
  address: string | null,
  device: string | null,
): { said: string; whole: string | null } {
  // A device not known yet is named not at all, never guessed (#2050).
  const on = device === null ? "" : ` on ${device}`;
  if (address === null || address === "")
    return { said: device === null ? "" : `On ${device}`, whole: null };
  if (address === "about:blank") return { said: `A blank page${on}`, whole: null };
  let host: string;
  try {
    host = new URL(address).host;
  } catch {
    host = "";
  }
  return { said: `${host === "" ? address : host}${on}`, whole: address };
}

/**
 * **The origin of `address`** (#2300): `http://localhost:5173`, for an http or https address,
 * and `null` for any other or one that does not read. What a chip names, and never more.
 */
export function originOfAddress(address: string): string | null {
  try {
    const url = new URL(address);
    return url.protocol === "http:" || url.protocol === "https:" ? url.origin : null;
  } catch {
    return null;
  }
}

/** The address each browser the window opened is to go to once its view is live, by program. */
const goingWhenShown = new Map<string, string>();

/**
 * **Browser `program` goes to `address` once its view is live** (#2176): as if the person typed
 * it there, so a site new to the project is asked about in the view. What opens a browser at an
 * address (the browsers open at the last quit, #2022) says so before it shows the view.
 */
export function goWhenShown(program: string, address: string): void {
  goingWhenShown.set(program, address);
}

/** The address browser `program` is to go to, once: none after it was taken. */
export function takeGoWhenShown(program: string): string | undefined {
  const address = goingWhenShown.get(program);
  goingWhenShown.delete(program);
  return address;
}
