// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Terminal } from "@xterm/xterm";
import type { Offer } from "./actions";
import type { GroupDrawn } from "./menuModel";

/**
 * **What a right-click on a chat's or a shell's terminal offers of the terminal's own**: Copy,
 * Paste and Select all, in the order iTerm2, VS Code's terminal and Windows Terminal list them.
 *
 * **Rows, and not catalogue offers** (`actions.ts`). A catalogue row is about the window — a
 * tab, a workspace, a chat — and the palette lists it. These are about what is selected in one
 * terminal at the moment the pointer came down, which the palette cannot name, so they are
 * declared here and drawn first in the terminal's menu; the pane's own rows, which are the
 * catalogue's, follow under a line (`SessionPane.tsx`).
 *
 * **Paste reads the clipboard in the core** ({@link ClipboardIn}), and types what it holds by
 * xterm's own paste, the keyboard's path.
 *
 * **No Clear.** xterm's `clear()` empties the screen as well as the history, and the session's
 * history is the core's, which a pane drawn again is sent whole: a row that seemed to clear it
 * would be undone by switching tabs. **No Open link or Copy link** either: a pane registers no
 * link provider, so there is no link the terminal knows it was clicked on.
 */
export type TerminalRow = {
  id: TerminalRowId;
  /** What the row reads, and its accessible name. */
  title: string;
  /** The key that does the same, where this window has one: drawn at the row's end. */
  keys?: string;
  available: boolean;
  /** Why it cannot run: non-empty exactly when `available` is false. */
  reason: string;
};

export type TerminalRowId = "terminal.copy" | "terminal.paste" | "terminal.selectAll";

const IDS: readonly string[] = ["terminal.copy", "terminal.paste", "terminal.selectAll"];

/** Whether `id` is one of the terminal's own rows, and not a row of the pane's. */
export function isTerminalRow(id: string): id is TerminalRowId {
  return IDS.includes(id);
}

/**
 * The terminal's rows as a menu draws them (`docs/ui-primitives.md`, "Menus": a menu whose rows
 * are not the catalogue's declares them the same way): one group, each row a `RowDrawn` around
 * an offer made for it, as `actions.fileRows` makes a file's. The offer carries the row's id,
 * whether it can run and why not; **what it does is not the catalogue's to carry out**
 * (`nothing`), and the terminal's menu reads the id and runs it on its own terminal.
 */
export function terminalGroup(rows: readonly TerminalRow[]): GroupDrawn {
  return {
    entries: rows.map((row) => ({
      kind: "row",
      offer: {
        id: row.id,
        title: row.title,
        available: row.available,
        reason: row.reason,
        does: { verb: "nothing" },
      } satisfies Offer,
      said: row.title,
      danger: false,
      keys: row.keys,
    })),
  };
}

/** Why Copy is off with nothing selected. */
export const NOTHING_SELECTED = "Nothing is selected.";

/**
 * The terminal's rows as they stand now: whether there is a selection, and whose keyboard it
 * is. Copy is off with nothing selected; Paste and Select all can always run, a terminal
 * drawn here taking input whenever it is drawn. **A key is shown only where the window has
 * it**: ⌘C and ⌘V are the Mac's Edit menu's, and ⌘A is xterm's own select-all there. Off a Mac
 * the window claims no key for any of the three (Ctrl+C is the program's), so none is drawn.
 */
export function terminalRows(now: { selected: boolean; mac: boolean }): TerminalRow[] {
  return [
    {
      id: "terminal.copy",
      title: "Copy",
      keys: now.mac ? "⌘C" : undefined,
      available: now.selected,
      reason: now.selected ? "" : NOTHING_SELECTED,
    },
    {
      id: "terminal.paste",
      title: "Paste",
      keys: now.mac ? "⌘V" : undefined,
      available: true,
      reason: "",
    },
    {
      id: "terminal.selectAll",
      title: "Select all",
      keys: now.mac ? "⌘A" : undefined,
      available: true,
      reason: "",
    },
  ];
}

/** Where Copy puts text: the page's clipboard, as the Brief's Copy and a Notice's do. */
export type ClipboardOut = { writeText: (text: string) => Promise<void> };

/**
 * How Paste reads the clipboard: **the core reads it, on the person's press**, and answers its
 * text, nothing where the clipboard holds no text, or a refusal in a sentence (the app's
 * `clipboard_text`, the window's alone; ADR 0052, amended 2026-10-14). The one seam the read
 * plugs in at: the page itself is given no way to read the clipboard.
 */
export type ClipboardIn = () => Promise<
  { status: "ok"; data: string | null } | { status: "error"; error: string }
>;

/** What the pane says of a Copy the clipboard refused. */
export const COPY_REFUSED = "the selection could not be put on the clipboard.";
/** What it says of a Copy with nothing left to copy. */
export const NOTHING_TO_COPY = "nothing is selected any more, so nothing was copied.";
/** What it says of a Paste from a clipboard that holds no text. */
export const NOTHING_TO_PASTE = "the clipboard holds no text, so nothing was pasted.";

/**
 * Carries a row out on `terminal`, and answers **what the pane is to say of it**, or nothing
 * where it went as asked.
 *
 * **Copy and Select all reach the program with nothing**: one reads the selection and the
 * other changes it, and neither is input. Copy puts on the clipboard exactly what the
 * keyboard's copy does — xterm's own `selectionText`, which `getSelection` returns and its
 * `copy` handler writes — and leaves the selection as it is. **The selection is read at the
 * press, not when the menu opened**: output that arrived while the menu was up may have
 * cleared it, and then nothing is written — an empty string would have emptied the clipboard.
 *
 * **Paste is the person typing, by the path their keyboard's paste takes**: the text goes to
 * xterm's own `paste()`, once, which brackets it when the program asked (so a paste of several
 * lines cannot run them one by one) and hands it on as everything typed is. Nothing else is
 * sent. A clipboard with no text, and a read the core refused, paste nothing and are said.
 */
export async function runTerminalRow(
  id: TerminalRowId,
  terminal: Pick<Terminal, "getSelection" | "selectAll" | "paste">,
  clipboard: { out: ClipboardOut | undefined; read: ClipboardIn },
): Promise<string | undefined> {
  switch (id) {
    case "terminal.selectAll":
      terminal.selectAll();
      return undefined;
    case "terminal.copy": {
      const selected = terminal.getSelection();
      if (selected === "") return NOTHING_TO_COPY;
      if (clipboard.out === undefined) return COPY_REFUSED;
      return clipboard.out.writeText(selected).then(
        () => undefined,
        () => COPY_REFUSED,
      );
    }
    case "terminal.paste": {
      const held = await clipboard.read().catch((err: unknown) => ({
        status: "error" as const,
        error: String(err),
      }));
      if (held.status === "error") return held.error;
      if (held.data === null || held.data === "") return NOTHING_TO_PASTE;
      terminal.paste(held.data);
      return undefined;
    }
  }
}

/**
 * **Whether the program in the terminal asked for the mouse** (`?1000h` and its kin): then a
 * click is the program's, a right-click among them, as in every terminal — a full-screen
 * program draws its own menus. Shift held is the standard way past it, and so is the menu key.
 */
export function programHasTheMouse(terminal: Pick<Terminal, "modes">): boolean {
  return terminal.modes.mouseTrackingMode !== "none";
}

/**
 * Whether this key asks for the menu: the menu key, or Shift+F10 with nothing else held.
 *
 * **The menu key takes nothing from the program**: xterm.js 6.0.0 sends no byte for it.
 * **Shift+F10 does** — `CSI 21;2~`, which a full-screen program may have a use for — so it is
 * the menu's only while no program has the mouse, the same line a right-click is drawn at: a
 * program that asked for the mouse draws its own menus, and gets the key as it gets the click.
 */
export function asksForTheMenu(event: KeyboardEvent, programHasMouse: boolean): boolean {
  if (event.ctrlKey || event.altKey || event.metaKey) return false;
  if (event.key === "ContextMenu") return true;
  return isShiftF10(event) && !programHasMouse;
}

/** Shift+F10 and nothing else held. */
export function isShiftF10(event: {
  key: string;
  shiftKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  metaKey: boolean;
}): boolean {
  return event.key === "F10" && event.shiftKey && !event.ctrlKey && !event.altKey && !event.metaKey;
}

/**
 * **What Shift+F10 is to a program**, as xterm.js 6.0.0 encodes it. A second Shift+F10, with
 * the menu the first one opened still up, closes the menu and sends this: the key is handed
 * back the way a second `F2` hands `F2` back from the palette (`docs/ui-primitives.md`).
 */
export const SHIFT_F10 = "\x1b[21;2~";

/**
 * **Whether this press is the pointer's way to a context menu**: the right button, or on a Mac
 * Ctrl with the main button, which the engine answers with `contextmenu` exactly as it does a
 * right-click.
 */
export function asksByPointer(event: { button: number; ctrlKey: boolean }, mac: boolean): boolean {
  return event.button === 2 || (mac && event.button === 0 && event.ctrlKey);
}

/**
 * Where the menu opens from the keyboard: under the terminal's cursor, in window coordinates.
 * The cell is the screen's box shared among its columns and rows, which is how xterm lays it
 * out; a terminal not yet drawn answers its holder's corner.
 */
export function underTheCursor(
  terminal: Pick<Terminal, "element" | "cols" | "rows" | "buffer">,
  holder: HTMLElement,
): { clientX: number; clientY: number } {
  const screen = terminal.element?.querySelector<HTMLElement>(".xterm-screen");
  const box = (screen ?? holder).getBoundingClientRect();
  if (!screen || terminal.cols === 0 || terminal.rows === 0)
    return { clientX: box.left, clientY: box.top };
  const cursor = terminal.buffer.active;
  return {
    clientX: box.left + (cursor.cursorX * box.width) / terminal.cols,
    clientY: box.top + ((cursor.cursorY + 1) * box.height) / terminal.rows,
  };
}
