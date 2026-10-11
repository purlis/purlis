// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A run of stacked combining marks, cut to a few** (#1146): the Inbox draws what an ask says
 * on a line that wraps, and a character carrying dozens of non-spacing marks draws them above
 * and below its line, over the rows around it.
 *
 * A few marks on one character are ordinary text, and are kept: a macOS path is in NFD, where
 * an accented letter is the letter and its marks (Vietnamese stacks two), and pointed Hebrew
 * stacks three or four. Past {@link MOST_MARKS} the rest of the run is dropped.
 *
 * Only the marks that draw over something are counted: non-spacing (Mn) and enclosing (Me).
 * A spacing mark (Mc) takes room of its own on the line, as a letter does.
 */

/** The most combining marks one character keeps. */
export const MOST_MARKS = 4;

const MARK = /[\p{Mn}\p{Me}]/u;

/** `text` with each run of combining marks past {@link MOST_MARKS} cut, and whether any was. */
export function capMarks(text: string, most = MOST_MARKS): { text: string; capped: boolean } {
  let out = "";
  let run = 0;
  let capped = false;
  for (const point of text) {
    if (!MARK.test(point)) {
      run = 0;
      out += point;
      continue;
    }
    run += 1;
    if (run > most) capped = true;
    else out += point;
  }
  return { text: capped ? out : text, capped };
}
