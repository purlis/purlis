// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from "react";

/**
 * **The browser needs you** (#2015, spec #2008): a chat asked the person to take one of its
 * browsers over (a CAPTCHA, a second factor, a consent page), and the Ask is under Needs you.
 *
 * Its one answer, **Take over**, takes the browser over in the session host and then asks the
 * project's window to open the browser's view, where the bar says the person drives and Done
 * hands it back. An Inbox row and a tab's Notice are drawn far from the window's tabs, so they
 * ask by an event the window hears, as Open on a proposed change does (`proposedChangeOpen.ts`).
 */
export const OPEN_BROWSER_ASK = "purlis-open-browser-taken-over";

/** The event the core tells each Ask on that ended with its browser: an Update (#2015). */
export const BROWSER_ASK_ENDED = "browser-ask-ended";

/** The id of Take over, as the core offers it. */
export const TAKE_OVER = "take-over";

/** The id of Open its view, offered once the person drives the browser: it sends nothing. */
export const OPEN_VIEW = "open-view";

/** What an answer asked of a project's window: which project, which browser. */
export type OpenBrowserAsk = { plane: string; program: string };

/** Asks the window of project `plane` to open browser `program`'s view. */
export function askOpenBrowser(plane: string, program: string): void {
  window.dispatchEvent(
    new CustomEvent<OpenBrowserAsk>(OPEN_BROWSER_ASK, { detail: { plane, program } }),
  );
}

/** Opens a browser's view through `open` whenever an answer of `plane`'s asks for it. */
export function useOpenBrowserAsked(plane: string, open: (program: string) => void): void {
  useEffect(() => {
    const heard = (event: Event) => {
      const ask = (event as CustomEvent<OpenBrowserAsk>).detail;
      if (ask?.plane === plane && typeof ask.program === "string" && ask.program !== "")
        open(ask.program);
    };
    window.addEventListener(OPEN_BROWSER_ASK, heard);
    return () => window.removeEventListener(OPEN_BROWSER_ASK, heard);
  }, [plane, open]);
}
