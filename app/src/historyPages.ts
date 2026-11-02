// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { IDisposable, Terminal } from "@xterm/xterm";

/**
 * **How many lines of history a chat's view opens with** (#4742, spec #4724): the screen and
 * the newest 200 lines, never the 5,000 a session keeps. Replaying all of them is what made
 * opening a busy chat's pane slow; the rest are paged in as the person scrolls up.
 */
export const FIRST_PAGE = 200;

/** How many more lines each page asks for as the person reaches the top of what the pane has. */
export const PAGE = 1000;

/**
 * **Asks for an older page once the person scrolls to the top of the history a view drew**
 * (#4742). `more` is called with the history the next view is to be drawn with, at most once
 * per view: a host with nothing older, such as one before 1.69, which draws all of it, is asked
 * for nothing.
 */
export function pageHistory(pane: Terminal, more: (history: number) => void) {
  let history = FIRST_PAGE;
  let older = 0;
  let asked = false;
  const ask = () => {
    const buffer = pane.buffer.active;
    if (asked || older === 0 || buffer.viewportY > 0 || buffer.length <= pane.rows) return;
    asked = true;
    more(history + PAGE);
  };
  const scrolled: IDisposable = pane.onScroll(ask);
  return {
    /** A view drew `drawn` lines of history, and the host keeps `left` lines above them. */
    opened(drawn: number, left: number) {
      history = drawn;
      older = left;
      asked = false;
    },
    /** The view it asked for did not open: the person may ask again by scrolling. */
    refused() {
      asked = false;
    },
    dispose: () => scrolled.dispose(),
  };
}

/**
 * **Keeps `line` in view while a view drawn with more history is written** (#4742): the lines
 * paged in go above the ones the person was reading, so what was at the top of the pane is now
 * `line` lines down. Written in pieces, the buffer grows from the top, and xterm follows the
 * bottom until the person's place exists, so it is put back after each piece until it does.
 */
export function keepInView(pane: Terminal, line: number) {
  let kept = false;
  return () => {
    if (kept) return;
    pane.scrollToLine(line);
    kept = pane.buffer.active.length >= line + pane.rows;
  };
}
