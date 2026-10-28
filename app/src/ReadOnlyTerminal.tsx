// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The terminal surface's read-only mode** (#3676, spec #2897, WCAG 2.2 AA): what a terminal
 * showed, drawn by xterm from the theme's terminal tokens and font, with no cursor and no input,
 * and **read as text**. A saved grade chat (`GradeScorecard.tsx`), the theme specimen's terminals,
 * and any later screen that is over (a finished task's last screen, a session record's replay)
 * are drawn by it. A module beside the pane's terminal (`SessionPane.tsx`), not a component
 * family: the terminal stays its own surface.
 *
 * - **Never only a picture** (WCAG 1.1.1, 1.3.1). The grid is what a sighted person sees and is
 *   hidden from a screen reader; beside it, the screen's own text, as xterm holds it, is a list of
 *   its lines (a row xterm wrapped is joined to the row it continues), so a reader walks it line by
 *   line. Nothing stands for the text with `role="img"`.
 * - **One Tab stop, named and described**: the screen is a region named by `label`, described by
 *   the words that say it is read-only, with the focus ring on its edge. xterm's own text box is
 *   never a Tab stop, and a click in the grid hands the keyboard back to the screen.
 * - **Find and copy from the keyboard.** ⌘F (Ctrl+F off a Mac) opens the window's find bar on it,
 *   which marks every match on the grid; ⌘A selects the whole screen and ⌘C copies what is
 *   selected, on the grid or in the text a reader selected. Esc in the find bar closes it and the
 *   keyboard comes back to the screen.
 * - **It says it is read-only in words, outside the grid** (`note`), as the prototype does.
 * - **Drawn as a terminal is**, from the theme in force or the one it is handed (`theme`), at the
 *   text size it is handed (the window's terminal size, or the specimen's fixed one), with no cursor
 *   and nothing typed reaching anything.
 */
import { useId, useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";
import { Terminal } from "@xterm/xterm";
import { SearchAddon } from "@xterm/addon-search";
import { FindBar } from "./FindBar";
import { opensFindInAFile } from "./keys/findKeys";
import { onAMac } from "./mac";
import { drawnFont, inForce, terminalFont, xtermLook, xtermTheme, type Theme } from "./theme/theme";

/** What a read-only screen says of itself when its owner says nothing more. */
export const READ_ONLY_SAID = "Read-only: it takes no input.";

/** Where a copy is written: the page's clipboard, as the Brief's Copy and a Notice's are. */
export type ClipboardOut = Pick<Clipboard, "writeText">;

export interface ReadOnlyTerminalProps {
  /** What the terminal showed: its bytes, escapes and all, as it was given them. */
  screen: string;
  /** What the screen is, as a region: "A grade's chat of deploy". */
  label: string;
  /** The words that say it is read-only, drawn outside the grid and describing it. */
  note?: string;
  /**
   * Whether the note is drawn. It is always said; only a photograph of a theme (the specimen),
   * which no one reads in place, keeps it out of the picture.
   */
  noteShown?: boolean;
  cols: number;
  rows: number;
  /** The theme it is drawn in: the one in force when not given. */
  theme?: Theme;
  /** Its text size: the window's terminal text size (`textSizes().terminal`), or a fixed one. */
  fontSize: number;
  /** Told once the screen is written, for a page that waits on it before it says it is drawn. */
  onWriting?: (written: Promise<void>) => void;
  /** A hook for its frame in a view, and for the tests that find it. Never its look. */
  className?: string;
  "data-testid"?: string;
  /** Where a copy goes: the page's clipboard unless a test says otherwise. */
  clipboard?: ClipboardOut;
}

/**
 * **The screen's lines, as text**: every row xterm holds, the scrollback with it, a row xterm
 * wrapped joined to the one it continues, blank rows left out, trailing spaces cut.
 */
export function linesOf(pane: Terminal): string[] {
  const buffer = pane.buffer.active;
  const lines: string[] = [];
  for (let at = 0; at < buffer.length; at += 1) {
    const row = buffer.getLine(at);
    if (!row) continue;
    const text = row.translateToString(true);
    if (row.isWrapped && lines.length > 0) lines[lines.length - 1] += text;
    else lines.push(text);
  }
  return lines.map((one) => one.trimEnd()).filter((one) => one !== "");
}

/** Whether `event` is the select-all or the copy chord: ⌘ on a Mac, Ctrl elsewhere. */
function chord(event: KeyboardEvent, key: "a" | "c", mac: boolean): boolean {
  if (event.key.toLowerCase() !== key || event.altKey || event.shiftKey) return false;
  return mac ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
}

/** **A read-only terminal**: the note, then the screen, drawn and read, with its find bar. */
export function ReadOnlyTerminal({
  screen,
  label,
  note = READ_ONLY_SAID,
  noteShown = true,
  cols,
  rows,
  theme,
  fontSize,
  onWriting,
  className,
  "data-testid": testId,
  clipboard,
}: ReadOnlyTerminalProps) {
  const noteId = useId();
  const holder = useRef<HTMLDivElement>(null);
  const region = useRef<HTMLElement>(null);
  const drawn = useRef<{ pane: Terminal; search: SearchAddon } | null>(null);
  const [lines, setLines] = useState<string[]>([]);
  const [finding, setFinding] = useState<{ search: SearchAddon; asked: number } | null>(null);
  // Read once, as it is drawn: a later write of the same screen is the same picture.
  const told = useRef(onWriting);
  useLayoutEffect(() => {
    told.current = onWriting;
  });
  // A layout effect, so the terminal is opened and its screen handed over in the commit the page
  // renders in, and a page that waits on it knows what to wait on.
  useLayoutEffect(() => {
    const where = holder.current;
    if (!where) return;
    const look = theme ?? inForce();
    const pane = new Terminal({
      fontSize,
      fontFamily:
        theme === undefined ? terminalFont() : (drawnFont("terminal", theme) ?? "monospace"),
      theme: xtermTheme(look),
      // Its line height and bold are the theme's; a screen that is over never blinks.
      ...xtermLook(look, true),
      cols,
      rows,
      cursorBlink: false,
      disableStdin: true,
      scrollback: 10_000,
      // The search addon draws every match as a decoration, which xterm.js 6.0.0 still calls
      // proposed API.
      allowProposedApi: true,
    });
    const search = new SearchAddon();
    pane.loadAddon(search);
    pane.open(where);
    // xterm's own text box is never a Tab stop: the screen's region is.
    pane.textarea?.setAttribute("tabindex", "-1");
    drawn.current = { pane, search };
    let live = true;
    // No cursor: what it showed is over.
    const written = new Promise<void>((done) =>
      pane.write(`${screen}\u001b[?25l`, () => {
        if (live) setLines(linesOf(pane));
        done();
      }),
    );
    told.current?.(written);
    return () => {
      live = false;
      drawn.current = null;
      setFinding(null);
      pane.dispose();
    };
  }, [screen, cols, rows, theme, fontSize]);

  const keys = (event: KeyboardEvent<HTMLElement>) => {
    const now = drawn.current;
    if (!now || event.defaultPrevented) return;
    const mac = onAMac();
    if (opensFindInAFile(event, mac)) {
      event.preventDefault();
      setFinding((open) => ({ search: now.search, asked: (open?.asked ?? 0) + 1 }));
    } else if (chord(event, "a", mac)) {
      event.preventDefault();
      now.pane.selectAll();
    } else if (chord(event, "c", mac)) {
      // Text a reader selected in the screen's lines is copied as the page copies any text.
      const picked = window.getSelection()?.toString() ?? "";
      if (picked !== "" || !now.pane.hasSelection()) return;
      event.preventDefault();
      void (clipboard ?? navigator.clipboard)?.writeText(now.pane.getSelection());
    }
  };
  const closeFind = () => {
    setFinding(null);
    region.current?.focus();
  };

  return (
    <div
      className={className ? `read-only-terminal ${className}` : "read-only-terminal"}
      data-testid={testId}
    >
      <p
        id={noteId}
        className={noteShown ? "read-only-terminal-note ui-text-secondary" : "sr-only"}
      >
        {note}
      </p>
      <div className="read-only-terminal-frame">
        <section
          ref={region}
          className="read-only-terminal-screen"
          aria-label={label}
          aria-describedby={noteId}
          tabIndex={0}
          onKeyDown={keys}
        >
          {/* What a sighted person sees; its text is the list beside it. A click in it hands the
            keyboard back to the screen, so the keys stay the screen's. */}
          <div
            ref={holder}
            className="read-only-terminal-grid"
            aria-hidden="true"
            onFocus={() => region.current?.focus({ preventScroll: true })}
          />
          <ol className="sr-only" aria-label={`${label}, line by line`}>
            {lines.map((line, at) => (
              <li key={at}>{line}</li>
            ))}
          </ol>
        </section>
        {finding && (
          <FindBar
            search={finding.search}
            asked={finding.asked}
            label={`Find in ${label}`}
            onClose={closeFind}
          />
        )}
      </div>
    </div>
  );
}
