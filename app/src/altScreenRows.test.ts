// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { Terminal } from "@xterm/xterm";
import { keepAltScreenToRows } from "./altScreenRows";

/** #5478's cause, in xterm.js 6.0.0 itself, and the line that closes it (`altScreenRows.ts`). */

/** Writes `text` and waits for xterm to have parsed it, noting what its parser threw. */
function parsing(terminal: Terminal) {
  const threw: unknown[] = [];
  const core = (terminal as unknown as { _core: { _inputHandler: { parse: Parse } } })._core;
  const handler = core._inputHandler;
  const parse = handler.parse.bind(handler);
  handler.parse = (...args) => {
    try {
      return parse(...args);
    } catch (err) {
      threw.push(err);
    }
  };
  const write = (text: string) => new Promise<void>((parsed) => terminal.write(text, parsed));
  return { threw, write };
}

/** A chat's terminal as the pane makes it: 32 rows, its history, the alternate screen kept to
 *  its rows. */
const chatTerminal = () => {
  const terminal = new Terminal({ cols: 80, rows: 32, scrollback: 5000, allowProposedApi: true });
  keepAltScreenToRows(terminal);
  return terminal;
};

/** The strip takes rows and gives them back, twice, around a program on the alternate screen:
 *  the sequence xterm.js 6.0.0 throws on, with the rows from #5478's lines. */
async function stripAroundAFullScreenProgram(
  terminal: Terminal,
  write: (text: string) => Promise<void>,
) {
  await write("a turn's output\r\n");
  terminal.resize(80, 19);
  await write("\x1b[?1049h" + "\r\n".repeat(40) + "\x1bM");
  terminal.resize(80, 32);
  terminal.resize(80, 27);
  await write("\x1b[27;1H");
  terminal.resize(80, 32);
  await write("\x1b[26;1Hthe input line");
}
type Parse = (...args: unknown[]) => unknown;

/** The rows of the active screen, as text. */
const screen = (terminal: Terminal) => {
  const buffer = terminal.buffer.active;
  return Array.from(
    { length: terminal.rows },
    (_, row) => buffer.getLine(buffer.baseY + row)?.translateToString(true) ?? null,
  );
};

describe("a program on the alternate screen while its pane changes height (#5478)", () => {
  it("draws text at the foot of a pane grown again, after its rows shrank on the normal screen", async () => {
    const terminal = chatTerminal();
    const { threw, write } = parsing(terminal);
    await write("a turn's output\r\n");
    // The waiting strip takes rows from the pane while the program is on the normal screen.
    terminal.resize(80, 19);
    // A full-screen program takes the alternate screen and writes past its foot, then puts its
    // cursor a row up, where a TUI's input line is.
    await write("\x1b[?1049h" + "\r\n".repeat(40) + "\x1bM");
    // The strip gives the rows back, and the program draws on its last row.
    terminal.resize(80, 32);
    await write("\x1b[32;1Hthe foot");

    expect(threw).toEqual([]);
    expect(terminal.buffer.active.type).toBe("alternate");
    expect(terminal.buffer.active.baseY).toBe(0);
    expect(screen(terminal).at(-1)).toBe("the foot");
    terminal.dispose();
  });

  it("draws a row the strip took and gave back twice, which xterm 6.0.0 threw on", async () => {
    const terminal = chatTerminal();
    const { threw, write } = parsing(terminal);
    await stripAroundAFullScreenProgram(terminal, write);

    expect(threw).toEqual([]);
    expect(screen(terminal)[25]).toBe("the input line");
    terminal.dispose();
  });

  it("keeps the alternate screen to the pane's rows however often the pane shrinks", async () => {
    const terminal = chatTerminal();
    const { threw, write } = parsing(terminal);
    terminal.resize(80, 4);
    await write("\x1b[?1049h" + "\r\n".repeat(40));

    expect(threw).toEqual([]);
    expect(terminal.buffer.active.length).toBe(4);
    expect(terminal.buffer.active.baseY).toBe(0);
    terminal.dispose();
  });

  // When this fails, xterm no longer loses the alternate screen's length, and `altScreenRows.ts`
  // can go: it is the reason that module exists.
  it("still throws in xterm's own print without it, so it is still needed", async () => {
    const terminal = new Terminal({ cols: 80, rows: 32, scrollback: 5000, allowProposedApi: true });
    const { threw, write } = parsing(terminal);
    await stripAroundAFullScreenProgram(terminal, write);

    expect(String(threw[0])).toMatch(/TypeError.*setCellFromCodepoint/);
    terminal.dispose();
  });
});
