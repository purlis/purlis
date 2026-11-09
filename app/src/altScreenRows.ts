// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { IDisposable, Terminal } from "@xterm/xterm";

/**
 * **The alternate screen holds no more lines than the terminal has rows** (#5478), which
 * xterm.js 6.0.0 lets slip.
 *
 * xterm empties the alternate screen whenever the normal one comes back, and its
 * `Buffer.resize` shrinks how many lines a buffer may hold only when the buffer holds some. So
 * a terminal made shorter while its program is on the normal screen leaves the empty alternate
 * screen able to hold as many lines as it had rows. A program that then takes the alternate
 * screen and writes past its foot gives it history it should never have, and a resize after
 * that leaves the foot of the screen below the last line it holds: the next text written there
 * is `InputHandler.print` reading a row that is not there (`TypeError: undefined is not an object
 * (evaluating 'd.setCellFromCodepoint')`). That is thrown inside xterm's write loop, which then
 * never runs again; the pane stops reporting what it drew, the host stops sending at its
 * watermark (#4733), and the pane is frozen half-drawn. A chat's pane changes height whenever
 * the waiting strip comes and goes, which is how a person met it.
 *
 * So after every resize, an empty alternate screen is given the length xterm gives one it
 * fills: the terminal's rows. Upstream's `Buffer.resize` still has the gap (xterm.js#5588 and
 * #5598 guard `print` instead, and drop the text); this is the line it lacks, from outside.
 * It reaches xterm's insides, so it does nothing unless they are as 6.0.0 has them, and
 * `altScreenRows.test.ts` fails if xterm stops needing it or it stops reaching them.
 */
export function keepAltScreenToRows(terminal: Terminal): IDisposable {
  return terminal.onResize(({ rows }) => {
    const lines = altLines(terminal);
    if (lines !== undefined && lines.length === 0 && lines.maxLength !== rows)
      lines.maxLength = rows;
  });
}

/** What the alternate screen's lines are, as xterm.js 6.0.0's `CircularList` has them. */
interface Lines {
  readonly length: number;
  maxLength: number;
}

/** The alternate screen's lines, or none if xterm's insides are not what 6.0.0 has. */
function altLines(terminal: Terminal): Lines | undefined {
  const core = (terminal as unknown as { _core?: { _bufferService?: { buffers?: unknown } } })
    ._core;
  const lines = (core?._bufferService?.buffers as { alt?: { lines?: unknown } } | undefined)?.alt
    ?.lines as Partial<Lines> | undefined;
  return typeof lines?.length === "number" && typeof lines.maxLength === "number"
    ? (lines as Lines)
    : undefined;
}
