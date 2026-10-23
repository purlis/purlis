// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent as ReactMouseEvent,
} from "react";
import { Channel } from "@tauri-apps/api/core";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import { Terminal } from "@xterm/xterm";
import { CHAT_KEYBOARD } from "./actions";
import * as bench from "./bench";
import { commands, type PlaneId } from "./bindings";
import { FindBar } from "./FindBar";
import { paneDrawn } from "./paneKeyboard";
import { RECONNECTING_SAID, useReconnecting } from "./reconnecting";
import { draw } from "./renderer";
import { onScreenReaderMode, screenReaderMode } from "./screenReader";
import { onAMac } from "./tabKeys";
import { moveAlong } from "./tabSequence";
import { TerminalMenu, type PaneMenu } from "./TerminalMenu";
import {
  asksByPointer,
  asksForTheMenu,
  programHasTheMouse,
  runTerminalRow,
  terminalRows,
  SHIFT_F10,
  underTheCursor,
  type TerminalRow,
  type TerminalRowId,
} from "./terminalRows";
import { onTextSizes, textSizes } from "./textSize";
import {
  inForce,
  onDrawn,
  onTerminalFont,
  terminalFont,
  xtermLook,
  xtermTheme,
} from "./theme/theme";
import { drawTerminalFont } from "./theme/terminalFont";
import { scrollByDistance } from "./wheel";
import { useProjectShown } from "./projectShown";

/**
 * One pane, drawing one session.
 *
 * A terminal here exists only while the pane is drawn. It opens a view of the session,
 * which is sent the screen the session already has and then its output, so a pane can come and
 * go without the session noticing — the core has held its terminal all along.
 *
 * **A pane of a project that is not in front stays drawn** (`projectShown.tsx`): its terminal
 * and its view stay open and go on taking what the program writes, so the project comes back
 * with its screen as it is now and nothing to start. It is not fitted and takes no keyboard
 * until then.
 *
 * **A right-click on it opens the terminal's menu** (`TerminalMenu.tsx`): Copy, Paste and
 * Select all, then the pane's own rows. While the program has asked for the mouse a click is
 * the program's, a right-click too, and Shift held is the way past it, as in every terminal.
 * The menu key opens the menu under the cursor whichever it is, and Shift+F10 does while no
 * program has the mouse.
 *
 * **A chat on a runner whose link is down says "Reconnecting…" over its last screen** (#2496),
 * quietly, until the core says it is reconnected: the link is back, or it drew again from a new
 * snapshot.
 */
export function SessionPane({
  plane,
  session,
  focused,
  onFocus,
  menu,
}: {
  /** The plane this session belongs to. A session number is only half of a chat's name —
   *  every plane numbers its own from one — so it travels with every call this pane makes. */
  plane: PlaneId;
  session: number;
  focused: boolean;
  onFocus: () => void;
  /** The pane's own rows, listed in the terminal's menu under its own. None where the pane is
   *  drawn without a catalogue: the menu is then the terminal's rows alone. */
  menu?: PaneMenu;
}) {
  const holder = useRef<HTMLDivElement>(null);
  const terminal = useRef<Terminal | undefined>(undefined);
  const reconnecting = useReconnecting(plane, session);
  /** The terminal's rows as they stood when its menu opened: whether anything was selected is
   *  read then, once, and not while the menu is up. */
  const [rows, setRows] = useState<TerminalRow[]>([]);
  /** While the menu is being asked for from the keyboard, which the program's hold on the
   *  mouse does not stand in the way of. */
  const fromKeys = useRef(false);
  /** Says something in the pane, in the terminal's own hand, while it is up. */
  const saying = useRef<(trouble: string) => void>(() => {});
  /** The find bar, while it is open: the terminal's search, and how many times it has been
   *  asked for — a second `⌘F` puts the keyboard back in the field of a bar already open. */
  const [finding, setFinding] = useState<{ search: SearchAddon; asked: number } | null>(null);

  /** Esc, or the bar's close: nothing is left drawn, and the keyboard is the chat's again. */
  const closeFind = useCallback(() => {
    setFinding(null);
    terminal.current?.focus();
  }, []);

  /** Clicking a pane puts the keyboard in it, which is the whole point of clicking it. Both
   *  events are handled: a person's press arrives as `mousedown`, and something driving the
   *  window from outside — a scenario test — may only send `click`. */
  const take = useCallback(() => {
    terminal.current?.focus();
    onFocus();
  }, [onFocus]);

  /** The newest `take`, for the listener below, which is added once for the pane's life. */
  const taking = useRef(take);
  useEffect(() => {
    taking.current = take;
  }, [take]);

  /** A menu about to open reads what is selected; one that closed leaves nothing behind. */
  const menuOpens = useCallback((open: boolean) => {
    if (open)
      setRows(terminalRows({ selected: terminal.current?.hasSelection() ?? false, mac: onAMac() }));
  }, []);

  /** **A click the program asked for is the program's** (`programHasTheMouse`): xterm has
   *  already reported the press, and preventing the default here opens no menu, this one or
   *  the panes'. Shift held, or the keyboard, is past that. */
  const asked = useCallback((event: ReactMouseEvent<HTMLElement>) => {
    const pane = terminal.current;
    if (!pane || fromKeys.current || event.shiftKey) return;
    if (programHasTheMouse(pane)) event.preventDefault();
  }, []);

  /** Carries one of the terminal's rows out, and says in the pane what it has to say of it:
   *  a clipboard that refused, a Copy with nothing left to copy, a Paste with nothing to
   *  paste. **Paste's text is read by the core** (`commands.clipboardText`, the window's
   *  alone) and typed by the terminal's own paste. */
  const run = useCallback((id: TerminalRowId) => {
    const pane = terminal.current;
    if (!pane) return;
    void runTerminalRow(id, pane, {
      out: navigator.clipboard,
      read: commands.clipboardText,
    }).then((trouble) => {
      if (trouble !== undefined) saying.current(trouble);
    });
  }, []);

  /** The menu closed on a row of the terminal's own, or on Escape: the keyboard is the
   *  terminal's again, said here and not left to where the engine thinks it was. */
  const giveBack = useCallback(() => terminal.current?.focus(), []);

  /** **A second Shift+F10, with the menu up, is the program's** (`SHIFT_F10`): through the
   *  terminal's own input, the one path everything typed takes. */
  const passTheKey = useCallback(() => terminal.current?.input(SHIFT_F10), []);

  // A pane that becomes the focused one — by a split, or by its tab coming back — takes the
  // keyboard without being clicked. **And so does the focused one of a project that comes back
  // to the front**: its pane was kept, so nothing is opened that would take it.
  const shown = useProjectShown();
  useEffect(() => {
    if (focused && shown) terminal.current?.focus();
  }, [focused, shown]);

  useEffect(() => {
    const where = holder.current;
    if (!where) return;

    const pane = new Terminal({
      // The machine's terminal text size (charter-app#283), 13px unless the operator changed it.
      fontSize: textSizes().terminal,
      // The theme's terminal font, or the person's in its place (#2326): the engine wrote the
      // family list, and core's is today's.
      fontFamily: terminalFont(),
      // The same theme the rest of the window is drawn from. It used to be two hex values
      // written out here, which were `--paper` and `--ink` spelled a second time and had
      // nothing keeping them equal — and which left the other eighteen colours a terminal
      // has to the library's defaults.
      theme: xtermTheme(inForce()),
      // The cursor's shape and blink, the line height and bold in the bright colours are the
      // theme's too (#2327).
      ...xtermLook(inForce()),
      // The search addon draws every match as a decoration, which xterm.js 6.0.0 still calls
      // proposed API (`registerDecoration` throws without this). Nothing else here uses it.
      allowProposedApi: true,
      // **Screen-reader mode, when the person turned it on** (DS-6, #629): xterm then keeps
      // its rows as a list a screen reader reads, and says what is written as it arrives.
      screenReaderMode: screenReaderMode(),
    });
    const fitting = new FitAddon();
    pane.loadAddon(fitting);
    /**
     * Fits the terminal to its pane, **only while the pane is in the window**. A project that
     * is not in front keeps its panes in an element that is in no document (`projectShown.tsx`),
     * where a pane has no size: fitting it there would tell the program its screen is two
     * columns wide. Its size is the one it was left at, and it is fitted again when it is back
     * and the pane's size is known (the observer below hears that).
     */
    const fit = {
      fit: () => {
        if (where.isConnected) fitting.fit();
      },
    };
    const search = new SearchAddon();
    pane.loadAddon(search);
    /** What Shift+Enter sends, once the view says which harness this is — none for a shell,
     *  which keeps the terminal's own Enter (`Harness::newline`). */
    let newline: string | undefined;
    // **The two keys a chat's terminal answers itself** (SI-4), decided before xterm encodes
    // them. Each is swallowed on every phase — keydown, keypress, keyup — so xterm sends
    // nothing of its own for it, and acted on once, at keydown.
    pane.attachCustomKeyEventHandler((event) => {
      if (opensFind(event)) {
        if (event.type === "keydown") {
          event.preventDefault();
          setFinding((open) => ({ search, asked: (open?.asked ?? 0) + 1 }));
        }
        return false;
      }
      // **The menu key and Shift+F10 open the terminal's menu, under its cursor**, with the
      // event a right-click sends (`ContextMenu` in `ui/menu` opens on it as on a right-click).
      // Shift+F10 stays the program's while it has the mouse (`asksForTheMenu`).
      if (asksForTheMenu(event, programHasTheMouse(pane))) {
        // A key held down asks once: its repeats are swallowed with it.
        if (event.type === "keydown" && !event.repeat) {
          event.preventDefault();
          fromKeys.current = true;
          try {
            where.dispatchEvent(
              new MouseEvent("contextmenu", {
                bubbles: true,
                cancelable: true,
                ...underTheCursor(pane, where),
              }),
            );
          } finally {
            fromKeys.current = false;
          }
        }
        return false;
      }
      if (newline !== undefined && isShiftEnter(event)) {
        if (event.type === "keydown") {
          event.preventDefault();
          // Through the terminal's own input, so it reaches the program by the one path
          // everything typed does (`onData` below).
          pane.input(newline);
        }
        return false;
      }
      return true;
    });
    pane.open(where);
    // **Shift and a right-click are the menu's, whatever the program asked for.** On a Mac
    // xterm reports a press to a program that has the mouse with Shift held too, Shift among
    // the modifiers (elsewhere Shift is xterm's own way past the program), so the press is
    // stopped on its way down, before xterm's listener on the element it drew. React's
    // listeners are past that point too, so the pane takes the keyboard from here.
    //
    // **And its default is prevented, as xterm's own listener would have** (`preventDefault();
    // focus()`): the engine's default for a press on what cannot take focus is to take the
    // keyboard away from whatever has it, and the menu would then give it back to nothing
    // when it closed. Preventing it does not stop the `contextmenu` that follows.
    //
    // A Mac's Ctrl+click is the same gesture as the right button (`asksByPointer`).
    const pastTheProgram = (event: MouseEvent) => {
      if (!asksByPointer(event, onAMac()) || !event.shiftKey || !programHasTheMouse(pane)) return;
      event.stopPropagation();
      event.preventDefault();
      taking.current();
    };
    where.addEventListener("mousedown", pastTheProgram, true);
    // **The wheel scrolls by the distance moved** (SI-4), in the history and to a program that
    // tracks the mouse alike — not xterm's own answer, which is slow to start (`wheel.ts`).
    const wheeling = scrollByDistance(pane, where);
    // **A theme drawn while the pane is up is the pane's theme too** (M6.7). xterm is handed an
    // object, not a stylesheet, so nothing about the custom properties changing reaches it: the
    // window would repaint and the terminal — the thing the operator stares at — would not.
    // `pane.options.theme` is xterm's own way to change it on a live terminal. The strip under
    // the last row needs nothing here: it is `App.css`'s, read from `--terminal-background`,
    // now that xterm's stylesheet sits in a layer below charter's (M6.8).
    const unfollow = onDrawn((theme) => {
      pane.options.theme = xtermTheme(theme);
      const look = xtermLook(theme);
      const lineHeightChanged = look.lineHeight !== pane.options.lineHeight;
      Object.assign(pane.options, look);
      // A new line height is a new cell size, so the rows that fit the pane are new.
      if (lineHeightChanged) fit.fit();
    });
    // **And so is a text size changed while it is up** (charter-app#283), then a refit: the
    // cells are a new size, so the rows and columns that fit the pane are new, and `fit` is
    // what tells the program (`onResize` below). The window's own size is not the terminal's.
    let drawnAt = pane.options.fontSize;
    const unsize = onTextSizes(({ terminal: size }) => {
      if (size === drawnAt) return;
      drawnAt = size;
      pane.options.fontSize = size;
      fit.fit();
    });
    // **And so is the terminal's font** (#2326): another theme's, the person's, or a face that
    // has just arrived, which has xterm measure its cells again (`terminalFont.ts`); a refit
    // follows, as for a size.
    const unfont = onTerminalFont((family, arrived) => {
      if (drawTerminalFont(pane, family, arrived)) fit.fit();
    });
    // **And so is screen-reader mode turned on or off** while it is up: `options` is xterm's
    // own way to change it on a live terminal, which builds or drops the rows it keeps.
    const unread = onScreenReaderMode((on) => {
      pane.options.screenReaderMode = on;
    });
    terminal.current = pane;
    // A tab switched to this chat owes its terminal the keyboard (#1486, `paneKeyboard.ts`).
    const undrawn = paneDrawn(plane, session, () => pane.focus());
    bench.paneOpened(session, pane);

    let gone = false;
    const say = (trouble: string) => {
      if (!gone) pane.write(`\r\n\x1b[31mcharter: ${trouble}\x1b[0m\r\n`);
    };
    saying.current = say;
    // The renderer loads its own code, so a pane draws with the DOM until it is there.
    void draw(pane, say, () => !gone).then((drawing) => bench.paneDrawing(pane, drawing));
    // Input refused is worth seeing: the program has stopped reading it, or has ended.
    const sayIfRefused = (
      sending: Promise<{ status: "ok" } | { status: "error"; error: string }>,
    ) =>
      void sending.then(
        (sent) => {
          if (sent.status === "error") say(sent.error);
        },
        (err: unknown) => say(String(err)),
      );
    const typed = pane.onData((text) => sayIfRefused(commands.sendInput(plane, session, text)));
    // **And what is not text, as the bytes it is** (charter#493): xterm hands a mouse report in
    // the default encoding (`?1000h` without `?1006h`) to `onBinary`, one character per byte.
    // Sent as text, a column or row past 95 — a byte above 127 — would reach the program as two.
    const bytes = pane.onBinary((binary) =>
      sayIfRefused(
        commands.sendInputBytes(
          plane,
          session,
          Array.from(binary, (char) => char.charCodeAt(0) & 0xff),
        ),
      ),
    );
    // The terminal decides the size, and the program is told it.
    const resized = pane.onResize(({ cols, rows }) => {
      void commands.resizeSession(plane, session, cols, rows);
    });
    const watching = new ResizeObserver(() => fit.fit());
    watching.observe(where);

    let view: number | undefined;
    // What the view is sent waits here until the terminal is the size that screen was drawn
    // for, so a line the session wrapped is not wrapped again somewhere else. A benchmark
    // wants to know when its text reached the grid, so what it gives back travels with the
    // text it belongs to.
    const waiting: { text: string; written?: () => void }[] = [];
    let ready = false;
    const output = new Channel<string>();
    output.onmessage = (text) => {
      if (gone) return;
      const written = bench.paneSent(pane, text);
      if (ready) pane.write(text, written);
      else waiting.push({ text, written });
    };
    // The pane's own size first: a session nobody is showing keeps whatever size it had.
    void (async () => {
      fit.fit();
      const opened = await commands
        .watchSession(plane, session, output)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (opened.status === "error") {
        say(opened.error);
        return;
      }
      if (gone) {
        void commands.unwatchSession(plane, session, opened.data.view);
        return;
      }
      view = opened.data.view;
      // The core keeps the history; the pane keeps the same, so scrolling back shows what the
      // session has rather than what this terminal happens to have seen.
      pane.options.scrollback = opened.data.scrollback;
      newline = opened.data.newline ?? undefined;
      pane.resize(opened.data.columns, opened.data.rows);
      ready = true;
      for (const held of waiting.splice(0)) pane.write(held.text, held.written);
      // Now that the screen is drawn, the pane's own size applies again.
      fit.fit();
    })();

    return () => {
      gone = true;
      where.removeEventListener("mousedown", pastTheProgram, true);
      undrawn();
      unfollow();
      unsize();
      unfont();
      unread();
      watching.disconnect();
      wheeling.dispose();
      typed.dispose();
      bytes.dispose();
      resized.dispose();
      if (view !== undefined) void commands.unwatchSession(plane, session, view);
      pane.dispose();
      terminal.current = undefined;
      setFinding(null);
      bench.paneClosed(session, pane);
    };
  }, [plane, session]);

  return (
    <>
      <TerminalMenu
        rows={rows}
        onRun={run}
        pane={menu}
        onAsked={asked}
        onOpenChange={menuOpens}
        onGiveBack={giveBack}
        onPassKey={passTheKey}
      >
        <div
          className={["pane", focused && "focused", reconnecting && "reconnecting"]
            .filter(Boolean)
            .join(" ")}
          data-testid="pane"
          // Everything under here is a shell's keyboard, and the window's own bindings stand
          // back from the chords a terminal encodes (`actions.CHAT_KEYBOARD`, charter-app#106).
          {...{ [CHAT_KEYBOARD]: "" }}
          data-session={session}
          onKeyDownCapture={leavesTheChat}
          onMouseDown={take}
          onClick={take}
          ref={holder}
        />
      </TerminalMenu>
      {/* Always there, so a screen reader hears the word as it comes and goes. */}
      <div className="pane-reconnecting">
        <span
          className={reconnecting ? "pane-reconnecting-said ui-text-secondary" : undefined}
          role="status"
        >
          {reconnecting ? RECONNECTING_SAID : ""}
        </span>
      </div>
      {finding && <FindBar search={finding.search} asked={finding.asked} onClose={closeFind} />}
    </>
  );
}

/** Screen-reader mode (DS-6, #629): kept in the layout file by `screenReader.ts`, and said
 *  from here too, where every chat's terminal follows it. */
export { screenReaderMode, setScreenReaderMode } from "./screenReader";

/**
 * **Whether this key opens the find bar: `⌘F` on a Mac, `Ctrl+Shift+F` everywhere else.**
 *
 * One modifier per platform, as the text-size keys are (`textSize.sizeKey`), and for the rule
 * `docs/ui-primitives.md` holds every claimed key to. xterm.js 6.0.0 sends nothing for a
 * `⌘`-chord but `⌘A`, so `⌘F` takes nothing from the chat. `Ctrl+F` would: it is `\x06`,
 * readline's forward-char, in the shell every chat starts in — so off a Mac the chord adds
 * `Shift`, as GNOME Terminal and Konsole do for their own find, and xterm sends nothing for a
 * `Ctrl+Shift` letter. A Mac's `Ctrl+F` stays the shell's too.
 */
export function opensFind(event: globalThis.KeyboardEvent, mac: boolean = onAMac()): boolean {
  if (event.key !== "f" && event.key !== "F") return false;
  if (event.altKey) return false;
  return mac
    ? event.metaKey && !event.ctrlKey && !event.shiftKey
    : event.ctrlKey && event.shiftKey && !event.metaKey;
}

/** Shift+Enter and nothing else held. xterm.js 6.0.0 ignores Shift on Enter and sends a bare
 *  CR — the same byte as Enter, which is why every harness submitted on it. */
function isShiftEnter(event: globalThis.KeyboardEvent): boolean {
  return (
    event.key === "Enter" && event.shiftKey && !event.ctrlKey && !event.altKey && !event.metaKey
  );
}

/**
 * **How the keyboard leaves a chat's terminal: Ctrl+Tab forwards, Ctrl+Shift+Tab backwards**
 * (charter-app#189).
 *
 * Tab inside a terminal is the shell's — completion — and Shift+Tab is a harness's own
 * (Claude Code cycles its modes on it), so neither is ever taken: xterm prevents both and sends
 * them on, which is why a terminal is a Tab stop you arrive at and cannot Tab out of.
 *
 * **Ctrl+Tab is the platforms' own answer to exactly that**, a control that keeps Tab for
 * itself: GTK moves the focus out of a text view with it, as Windows does out of a multi-line
 * box and AppKit out of a field editor that took Tab. And it takes nothing from the chat, by
 * the rule `docs/ui-primitives.md` holds every claimed key to: xterm.js 6.0.0's
 * `evaluateKeyboardEvent` ignores Ctrl on key code 9 and sends `\t` for Ctrl+Tab and `ESC [ Z`
 * for Ctrl+Shift+Tab — the same bytes Tab and Shift+Tab already send. A shell cannot tell them
 * apart, so there is nothing to hand back.
 *
 * On the capture phase of the pane, so it runs before xterm's own listener on its textarea,
 * and stops the event there. WebKit has no default action for Ctrl+Tab (its tab handler
 * returns early on Ctrl), so where the keyboard goes is decided here, along the same sequence
 * Tab walks (`tabSequence.ts`).
 */
export function leavesTheChat(event: KeyboardEvent<HTMLElement>) {
  if (event.key !== "Tab" || !event.ctrlKey || event.metaKey || event.altKey) return;
  event.preventDefault();
  event.stopPropagation();
  moveAlong(event.currentTarget, event.shiftKey);
}
