// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent as ReactMouseEvent,
} from "react";
import { Channel } from "@tauri-apps/api/core";
import { PaneTerminalAt } from "./paneTerminal";
import { FitAddon } from "@xterm/addon-fit";
import { type OwnCells, TERMINAL_HOLDER, terminalInPane } from "./paneHeld";
import { afterTheSlide } from "./railSlide";
import { SearchAddon } from "@xterm/addon-search";
import { Terminal } from "@xterm/xterm";
import { CHAT_KEYBOARD } from "./actions";
import * as bench from "./bench";
import { commands, type PlaneId } from "./bindings";
import { typedInPane } from "./quiet";
import { FindBar } from "./FindBar";
import { paneDrawn } from "./paneKeyboard";
import { RECONNECTING_SAID, useReconnecting } from "./reconnecting";
import { draw } from "./renderer";
import { onScreenReaderMode, screenReaderMode } from "./screenReader";
import { onAMac } from "./tabKeys";
import { opensFind } from "./keys/findKeys";
import { LINK_CORE } from "./paneLinkCore";
import {
  linkPane,
  openLink,
  relayed,
  type LinkedPane,
  type LinkHands,
  type ScreenLink,
} from "./paneLinks";
import { usePaneLinks } from "./PaneLinkSurfaces";
import { isKey } from "./keys/registry";
import { moveAlong } from "./tabSequence";
import { TerminalMenu, type PaneMenu } from "./TerminalMenu";
import {
  asksByPointer,
  asksForTheMenu,
  programHasTheMouse,
  isLinkRow,
  LINK_COPY_REFUSED,
  linkRows,
  runTerminalRow,
  terminalRows,
  SHIFT_F10,
  underTheCursor,
  type ClipboardOut,
  type LinkRowId,
  type LinkRows,
  type TerminalRow,
  type TerminalRowId,
} from "./terminalRows";
import { onTextSizes, textSizes } from "./textSize";
import {
  inForce,
  linkColour,
  onDrawn,
  onTerminalFont,
  terminalFont,
  xtermLook,
  xtermTheme,
} from "./theme/theme";
import { drawTerminalFont } from "./theme/terminalFont";
import { scrollByDistance } from "./wheel";
import { drawnReports, writePerFrame } from "./paneWrites";
import { watchForNoEcho } from "./noEcho";
import { watchWrites } from "./stuckWrites";
import { keepAltScreenToRows } from "./altScreenRows";
import { useProjectShown } from "./projectShown";
import { viewOpening } from "./slowLog";
import { FIRST_PAGE, keepInView, pageHistory } from "./historyPages";

/** A full reset (RIS): what a pane is written before the screen of a view opened again. */
const RESET = new TextEncoder().encode("\x1bc");

import {
  chatShown,
  keepTerminal,
  keptKey,
  letGoOfKeptTerminal,
  takeKeptTerminal,
  type KeptTerminal,
} from "./keptTerminals";

/**
 * One pane, drawing one session.
 *
 * A terminal here opens a view of the session, which is sent the screen the session already
 * has and then its output, so a pane can come and go without the session noticing — the core
 * has held its terminal all along.
 *
 * **The last three chats shown keep their terminal when the pane leaves them**
 * (`keptTerminals.ts`, #4735): off the window, with their view open, taking what the program
 * writes. Switching back to one puts that terminal in the pane as it is, so nothing is started
 * and nothing is sent again. A chat further back is let go of, and showing it starts a terminal.
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
  readOnly = false,
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
  /** **The chat's program has ended** (#4756): its last screen stays, and takes no input. */
  readOnly?: boolean;
}) {
  const holder = useRef<HTMLDivElement>(null);
  const terminal = useRef<Terminal | undefined>(undefined);
  /** The terminal and its view this pane draws now, while it draws one. */
  const shownLive = useRef<Live | undefined>(undefined);
  /** **How many times the terminal was started again** (`noEcho.ts`): its view was stuck. */
  const [started, setStarted] = useState(0);
  /** Set as the terminal is to be let go of, not kept, as it leaves the pane. */
  const lettingGo = useRef(false);
  /** The terminal let go of had the keyboard: the one started in its place takes it. */
  const hadTheKeyboard = useRef(false);
  const reconnecting = useReconnecting(plane, session);
  /** The terminal's rows as they stood when its menu opened: whether anything was selected is
   *  read then, once, and not while the menu is up. */
  const [rows, setRows] = useState<TerminalRow[]>([]);
  /** The link rows as they stood when the menu opened (#6219): over the link the pointer was on
   *  as the menu was asked for, read then, once. */
  const [linksRows, setLinksRows] = useState<LinkRows | undefined>(undefined);
  /** The link the pointer was on as the menu was asked for: none from the keyboard (D4). */
  const overLink = useRef<ScreenLink | undefined>(undefined);
  /** While the menu is being asked for from the keyboard, which the program's hold on the
   *  mouse does not stand in the way of. */
  const fromKeys = useRef(false);
  /** **Its links' surfaces** (`PaneLinkSurfaces.tsx`, #6215): a hovered link's tooltip, and the
   *  window's answer to one ⌘-clicked that did not simply open. An ask accepted opens the
   *  target it showed, resolved again by the core. */
  const {
    hands: linkHands,
    surfaces: linkSurfaces,
    list: openLinksList,
  } = usePaneLinks({
    from: { plane, session },
    core: LINK_CORE,
    colour: () => linkColour(inForce()),
    // *Links in this pane…* (#6220) reads the screen of the terminal this pane draws now.
    onScreen: () => shownLive.current?.links.onScreen() ?? Promise.resolve([]),
  });
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

  /** Whether the pane has its Links list to open (#6220). */
  const lists = openLinksList !== undefined;
  /** A menu about to open reads what is selected; one that closed leaves nothing behind. */
  const menuOpens = useCallback(
    (open: boolean) => {
      if (!open) return;
      setRows(terminalRows({ selected: terminal.current?.hasSelection() ?? false, mac: onAMac() }));
      setLinksRows(linkRows({ link: overLink.current, mac: onAMac(), lists }));
    },
    [lists],
  );

  /** **A click the program asked for is the program's** (`programHasTheMouse`): xterm has
   *  already reported the press, and preventing the default here opens no menu, this one or
   *  the panes'. Shift held, or the keyboard, is past that. */
  const asked = useCallback((event: ReactMouseEvent<HTMLElement>) => {
    // The keyboard has no pointer, so no link under it (D4).
    overLink.current = fromKeys.current ? undefined : shownLive.current?.links.under();
    const pane = terminal.current;
    if (!pane || fromKeys.current || event.shiftKey) return;
    if (programHasTheMouse(pane)) event.preventDefault();
  }, []);

  /** Carries one of the terminal's rows out, and says in the pane what it has to say of it:
   *  a clipboard that refused, a Copy with nothing left to copy, a Paste with nothing to
   *  paste. **Paste's text is read by the core** (`commands.clipboardText`, the window's
   *  alone) and typed by the terminal's own paste. */
  /**
   * **A link row, on the link the menu opened over** (#6219). *Open link* goes the way ⌘-click
   * does: the core is handed the candidate's text and answers, asks included. *Copy link* puts
   * the target the core resolved on the clipboard. *Links in this pane…* opens the list (#6220).
   */
  const runLinkRow = useCallback(
    (id: LinkRowId) => {
      if (id === "terminal.links") return openLinksList?.();
      const link = overLink.current;
      if (link === undefined) return;
      if (id === "terminal.openLink") {
        if (linkHands !== undefined) void openLink(link.candidate, linkHands);
        return;
      }
      const out = navigator.clipboard as ClipboardOut | undefined;
      if (out === undefined) return saying.current(LINK_COPY_REFUSED);
      out.writeText(link.seen.target).catch(() => saying.current(LINK_COPY_REFUSED));
    },
    [openLinksList, linkHands],
  );

  const run = useCallback(
    (id: TerminalRowId) => {
      if (isLinkRow(id)) return runLinkRow(id);
      const pane = terminal.current;
      if (!pane) return;
      void runTerminalRow(id, pane, {
        out: navigator.clipboard,
        read: commands.clipboardText,
      }).then((trouble) => {
        if (trouble !== undefined) saying.current(trouble);
      });
    },
    [runLinkRow],
  );

  // **Its pane's head reaches it** (#4760): Copy and Paste on a right-click of the head run
  // here, as its own menu's do.
  const headRef = useContext(PaneTerminalAt);
  useEffect(() => {
    if (headRef === null) return;
    headRef.current = { selected: () => terminal.current?.hasSelection() ?? false, run };
    return () => {
      headRef.current = null;
    };
  }, [headRef, run]);

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

  // **A chat on screen is one of the last shown** (`keptTerminals.ts`), and is marked so before
  // its pane takes its terminal below: a chat drawn in a project off the window is not shown.
  useEffect(() => {
    if (shown) chatShown(keptKey(plane, session));
  }, [plane, session, shown]);

  useEffect(() => {
    const where = holder.current;
    if (!where) return;
    const key = keptKey(plane, session);

    // **The chat's own terminal, if it is one of the last shown** (`keptTerminals.ts`): it was
    // kept when the pane left it, has taken what its program wrote since, and is put back here
    // as it is. Otherwise a new one, which opens a view and is sent the screen.
    const kept = takeKeptTerminal<Live>(key);
    const live = kept ?? startTerminal(plane, session, where);
    const pane = live.pane;
    live.owner = {
      holder: where,
      fromKeys,
      links: linkHands,
      listLinks: () => openLinksList?.(),
      find: (search) => setFinding((open) => ({ search, asked: (open?.asked ?? 0) + 1 })),
      startAgain: () => {
        lettingGo.current = true;
        setStarted((times) => times + 1);
      },
    };
    if (pane.element !== undefined && pane.element.parentElement !== where)
      where.appendChild(pane.element);
    terminal.current = pane;
    shownLive.current = live;
    saying.current = live.say;
    // **A terminal started again takes the keyboard its last one had** (`noEcho.ts`): it is
    // started while its person types, and their next key goes to it.
    if (hadTheKeyboard.current) pane.focus();
    hadTheKeyboard.current = false;
    if (kept !== undefined) bench.paneShownAgain(session, pane);

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
    // A tab switched to this chat owes its terminal the keyboard (#1486, `paneKeyboard.ts`).
    const undrawn = paneDrawn(plane, session, () => pane.focus());
    // Once the project rail's slide is over, not on each of its frames (`railSlide.ts`): the
    // program is told one new size per hide or show. Heard as the pane is first watched too, so
    // a kept terminal is fitted to the pane it is put back in.
    const refit = () => live.fit();
    // What a drag reckons the size this terminal would take from (`panePreview.gridIn`).
    const unmeasured = terminalInPane(where, () => ownCells(pane));
    let unqueue = () => {};
    const watching = new ResizeObserver(() => {
      unqueue = afterTheSlide(refit);
    });
    watching.observe(where);

    return () => {
      where.removeEventListener("mousedown", pastTheProgram, true);
      unmeasured();
      wheeling.dispose();
      undrawn();
      watching.disconnect();
      // A pane gone before the slide's end fits nothing at it.
      unqueue();
      live.owner = undefined;
      terminal.current = undefined;
      shownLive.current = undefined;
      setFinding(null);
      // **Kept if the chat is one of the last shown**, off the window and still taking what
      // its program writes; let go of otherwise, its view closed.
      // A terminal started again is let go of here, as one that falls out of the kept chats is.
      const lettingGoOf = lettingGo.current;
      lettingGo.current = false;
      hadTheKeyboard.current =
        lettingGoOf && pane.element?.contains(document.activeElement) === true;
      if (!lettingGoOf && keepTerminal(key, live)) pane.element?.remove();
      else live.release();
    };
  }, [plane, session, started, linkHands, openLinksList]);
  // **An ended chat's last screen takes no input** (#4756): xterm sends nothing typed, and the
  // screen is read, scrolled, found in and copied from as before. After the effect above, which
  // made the terminal it is set on.
  useEffect(() => {
    if (terminal.current) terminal.current.options.disableStdin = readOnly;
  }, [plane, readOnly, session, started]);

  return (
    <>
      <TerminalMenu
        rows={rows}
        links={linksRows}
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
          {...{ [TERMINAL_HOLDER]: "" }}
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
      {linkSurfaces}
    </>
  );
}

/** The scrollbar's width a terminal's fit leaves out (`@xterm/addon-fit`), where it has no ruler. */
const SCROLLBAR = 14;

/**
 * **A terminal's cell, and what its fit leaves out**, as `@xterm/addon-fit` reckons them: read from
 * what is drawn, its screen over its grid, and none before it has been laid out.
 */
function ownCells(pane: Terminal): OwnCells | undefined {
  const screen = pane.element?.querySelector(".xterm-screen")?.getBoundingClientRect();
  if (pane.element === undefined || screen === undefined || screen.width === 0) return undefined;
  const padding = getComputedStyle(pane.element);
  const side = (name: string) => parseInt(padding.getPropertyValue(name)) || 0;
  return {
    width: screen.width / pane.cols,
    height: screen.height / pane.rows,
    extraWidth:
      side("padding-left") +
      side("padding-right") +
      (pane.options.scrollback === 0 ? 0 : (pane.options.overviewRuler?.width ?? SCROLLBAR)),
    extraHeight: side("padding-top") + side("padding-bottom"),
  };
}

/** What a pane draws a terminal in, while one does: kept, a terminal has none. */
type Owner = {
  holder: HTMLElement;
  /** Set while the menu is asked for from the keyboard (`asked` in the pane). */
  fromKeys: { current: boolean };
  /** Opens the find bar on the terminal's search, or asks an open one for the keyboard. */
  find: (search: SearchAddon) => void;
  /** Lets go of the terminal and its view and starts both again (`noEcho.ts`). */
  startAgain: () => void;
  /** What its links' hover and answers are drawn by (`PaneLinkSurfaces.tsx`). */
  links: LinkHands | undefined;
  /** Opens *Links in this pane…* (#6220). */
  listLinks: () => void;
};

/**
 * **A chat's terminal and its view, for as long as either is drawn or kept**
 * (`keptTerminals.ts`): built by the first pane to show the chat, drawn by each pane that shows
 * it after, and let go of by {@link Live.release}.
 */
type Live = KeptTerminal & {
  pane: Terminal;
  /** The pane drawing it now. */
  owner: Owner | undefined;
  say: (trouble: string) => void;
  /** Fits it to the pane it is in, while it is in the window. */
  fit: () => void;
  /** Its link hook (`paneLinks.ts`): what *Links in this pane…* reads its screen by. */
  links: LinkedPane;
};

/**
 * Starts chat `session`'s terminal in `where`, opens a view of the session and draws what the
 * view is sent. What follows the window, a theme, a text size, a font or screen-reader mode,
 * is followed for as long as the terminal is, drawn or kept.
 */
function startTerminal(plane: PlaneId, session: number, where: HTMLElement): Live {
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
  // **Its alternate screen holds no more lines than it has rows** (#5478): xterm.js 6.0.0 lets
  // that slip on a resize, and its write loop then throws for good (`altScreenRows.ts`).
  const altScreenKept = keepAltScreenToRows(pane);
  const fitting = new FitAddon();
  pane.loadAddon(fitting);
  /**
   * Fits the terminal to its pane, **only while the pane is in the window**. A terminal kept
   * off it, a chat's or a project's (`keptTerminals.ts`, `projectShown.tsx`), has no size
   * there: fitting it would tell the program its screen is two columns wide. Its size is the
   * one it was left at, and it is fitted again when it is back and the pane's size is known
   * (the pane's observer hears that).
   */
  const fit = () => {
    if (!pane.element?.isConnected) return;
    fitting.fit();
    // **How tall its line is, for the pane's three-line floor** (#5215): the band over it gives
    // way before the terminal goes under three of them (`paneFrame.css`).
    const lines = pane.element.querySelector(".xterm-screen")?.clientHeight ?? 0;
    if (lines > 0 && pane.rows > 0)
      pane.element
        .closest<HTMLElement>(".pane-body")
        ?.style.setProperty("--terminal-row", `${lines / pane.rows}px`);
  };
  const search = new SearchAddon();
  pane.loadAddon(search);
  // **Its links** (#6215, `paneLinks.ts`): found under the pointer, decided by the core, drawn
  // by the pane showing it, whichever that is now.
  const links = linkPane(
    pane,
    relayed(() => live.owner?.links),
  );
  /** What Shift+Enter sends, once the view says which harness this is — none for a shell,
   *  which keeps the terminal's own Enter (`Harness::newline`). */
  let newline: string | undefined;
  const live: Live = {
    pane,
    links,
    owner: undefined,
    say: () => {},
    fit,
    release: () => {},
  };
  // **The two keys a chat's terminal answers itself** (SI-4), decided before xterm encodes
  // them. Each is swallowed on every phase — keydown, keypress, keyup — so xterm sends
  // nothing of its own for it, and acted on once, at keydown.
  pane.attachCustomKeyEventHandler((event) => {
    // **Links in this pane…** (#6220): ⇧⌘L, Ctrl+Shift+L off a Mac.
    if (isKey(event, "pane.links")) {
      if (event.type === "keydown" && !event.repeat) {
        event.preventDefault();
        live.owner?.listLinks();
      }
      return false;
    }
    if (isKey(event, "chat.find")) {
      if (event.type === "keydown") {
        event.preventDefault();
        live.owner?.find(search);
      }
      return false;
    }
    // **The menu key and Shift+F10 open the terminal's menu, under its cursor**, with the
    // event a right-click sends (`ContextMenu` in `ui/menu` opens on it as on a right-click).
    // Shift+F10 stays the program's while it has the mouse (`asksForTheMenu`).
    if (asksForTheMenu(event, programHasTheMouse(pane))) {
      // A key held down asks once: its repeats are swallowed with it.
      const owner = live.owner;
      if (owner !== undefined && event.type === "keydown" && !event.repeat) {
        event.preventDefault();
        owner.fromKeys.current = true;
        try {
          owner.holder.dispatchEvent(
            new MouseEvent("contextmenu", {
              bubbles: true,
              cancelable: true,
              ...underTheCursor(pane, owner.holder),
            }),
          );
        } finally {
          owner.fromKeys.current = false;
        }
      }
      return false;
    }
    if (newline !== undefined && isKey(event, "chat.newline")) {
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
  // **A theme drawn while the terminal is up is its theme too** (M6.7). xterm is handed an
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
    if (lineHeightChanged) fit();
  });
  // **And so is a text size changed while it is up** (charter-app#283), then a refit: the
  // cells are a new size, so the rows and columns that fit the pane are new, and `fit` is
  // what tells the program (`onResize` below). The window's own size is not the terminal's.
  let drawnAt = pane.options.fontSize;
  const unsize = onTextSizes(({ terminal: size }) => {
    if (size === drawnAt) return;
    drawnAt = size;
    pane.options.fontSize = size;
    fit();
  });
  // **And so is the terminal's font** (#2326): another theme's, the person's, or a face that
  // has just arrived, which has xterm measure its cells again (`terminalFont.ts`); a refit
  // follows, as for a size.
  const unfont = onTerminalFont((family, arrived) => {
    if (drawTerminalFont(pane, family, arrived)) fit();
  });
  // **And so is screen-reader mode turned on or off** while it is up: `options` is xterm's
  // own way to change it on a live terminal, which builds or drops the rows it keeps.
  const unread = onScreenReaderMode((on) => {
    pane.options.screenReaderMode = on;
  });
  bench.paneOpened(session, pane, plane);

  let gone = false;
  const say = (trouble: string) => {
    if (!gone) pane.write(`\r\n\x1b[31mcharter: ${trouble}\x1b[0m\r\n`);
  };
  live.say = say;
  // The renderer loads its own code, so a pane draws with the DOM until it is there.
  void draw(pane, say, () => !gone).then((drawing) => bench.paneDrawing(pane, drawing));
  // Input refused is worth seeing: the program has stopped reading it, or has ended.
  const sayIfRefused = (sending: Promise<{ status: "ok" } | { status: "error"; error: string }>) =>
    void sending.then(
      (sent) => {
        if (sent.status === "error") say(sent.error);
      },
      (err: unknown) => say(String(err)),
    );
  // **Typed into and sent nothing back, its view is stuck** (#5478, `noEcho.ts`): the pane starts
  // its terminal and view again, as falling out of the kept chats does.
  const echo = watchForNoEcho(
    keptKey(plane, session),
    () => pane.element?.isConnected === true,
    () => live.owner?.startAgain(),
  );
  const typed = pane.onData((text) => {
    echo.typed(text);
    // The person typing in a chat is Quiet for a while (#6017): no notification breaks in.
    typedInPane(text);
    sayIfRefused(commands.sendInput(plane, session, text));
  });
  // **And what is not text, as the bytes it is** (charter#493): xterm hands a mouse report in
  // the default encoding (`?1000h` without `?1006h`) to `onBinary`, one character per byte.
  // Sent as text, a column or row past 95 — a byte above 127 — would reach the program as two.
  const bytes = pane.onBinary((binary) => {
    sayIfRefused(
      commands.sendInputBytes(
        plane,
        session,
        Array.from(binary, (char) => char.charCodeAt(0) & 0xff),
      ),
    );
  });
  // The terminal decides the size, and the program is told it.
  const resized = pane.onResize(({ cols, rows }) => {
    void commands.resizeSession(plane, session, cols, rows);
  });

  let view: number | undefined;
  /** The view asked for last (#4742): the one drawn before it is let go of once it answers. */
  let asked = 0;
  /** The view whose output the pane draws: the last asked for that answered. Until the next
   *  one answers, the one before it is still attached and still drawn, so a page refused or
   *  slow to answer never leaves the pane showing nothing new. */
  let drawing = 0;
  /** How many lines of history the view in use left out, above those it drew. */
  let older = 0;
  // **Older history is paged in as the person scrolls up** (#4742): the view is opened again
  // with more, and the one before it let go of once the new one answers.
  const paging = pageHistory(pane, (history) => void open(history));
  // **Kept or drawn, it takes what the program writes** (`keptTerminals.ts`), so a chat shown
  // again has its screen as it is now and nothing to catch up.
  // What arrives within a frame is one write to the terminal, not one per chunk (#4732): one
  // for the terminal, whichever view it came from, so a page's reset stays in its order.
  // **A terminal that stops parsing what it is written starts again** (#5478, `stuckWrites.ts`):
  // its view would never be reported drawn again, and the host would send it nothing more.
  const watched = watchWrites(
    keptKey(plane, session),
    (bytes, parsed) => pane.write(bytes, parsed),
    () => {
      // One drawn starts again in its pane; one kept off the window is let go of, and starts
      // again when it is next shown.
      if (live.owner !== undefined) live.owner.startAgain();
      else letGoOfKeptTerminal(keptKey(plane, session));
    },
  );
  const writes = writePerFrame(watched.write);
  /** Ends the stall log's note of the view being opened (`slowLog.ts`). */
  let openingDone = () => {};
  // What the view is sent waits here until the terminal is the size that screen was drawn
  // for, so a line the session wrapped is not wrapped again somewhere else. A benchmark
  // wants to know when its text reached the grid, so what it gives back travels with the
  // text it belongs to.
  const open = async (history: number) => {
    const mine = ++asked;
    const again = view !== undefined;
    const waiting: { bytes: Uint8Array; written?: () => void }[] = [];
    /** Put back after each piece of a page is written, so the person keeps their place. */
    let keep = () => {};
    // What the terminal has drawn of this view is told back, one report per write, and the
    // session host paces the view by it (#4733): a pane that falls behind is sent a fresh
    // screen, not a backlog. A view a page has replaced is closed, so it is told nothing.
    const drawn = drawnReports((told) => {
      if (gone || mine !== drawing || view === undefined) return;
      void commands.sessionDrawn(plane, session, view, told);
    });
    // Each piece is the bytes the program wrote, a raw body over IPC rather than a string
    // escaped into JSON (#4745); the terminal decodes them, a character cut between two
    // pieces included.
    const output = new Channel<ArrayBuffer>();
    output.onmessage = (buffer) => {
      if (gone || (mine !== drawing && mine !== asked)) return;
      const bytes = new Uint8Array(buffer);
      if (mine === drawing) {
        echo.output();
      }
      const benched = bench.paneSent(pane, bytes);
      const written = () => {
        keep();
        benched?.();
        drawn.piece();
      };
      if (mine === drawing) writes.add(bytes, written);
      else waiting.push({ bytes, written });
    };
    // Opening the view, until its screen is handed to the terminal: what the stall log says
    // was going on (`slowLog.ts`). A page's view opened again counts too, and ends the
    // opening of the one it replaces, whose answer is let go of. A kept terminal's view still
    // opening is still going on.
    openingDone();
    const done = viewOpening();
    openingDone = done;
    // The pane's own size first: a session nobody is showing keeps whatever size it had.
    if (!again) fit();
    const opened = await commands
      .watchSession(plane, session, output, history)
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
    if (opened.status === "error") {
      done();
      if (again) paging.refused();
      else say(opened.error);
      return;
    }
    if (gone || mine !== asked) {
      done();
      void commands.unwatchSession(plane, session, opened.data.view);
      return;
    }
    // From here the new view is drawn and the one before it is not.
    drawing = mine;
    const before = view;
    view = opened.data.view;
    if (before !== undefined) {
      void commands.unwatchSession(plane, session, before);
      // The lines paged in go above the ones the person was reading.
      const above = pane.buffer.active.viewportY + Math.max(0, older - opened.data.older);
      keep = keepInView(pane, above);
      // Written, not `reset()`: after whatever of the old view is still queued to be parsed,
      // and so the full reset the core writes before a fresh snapshot of one view too.
      writes.add(RESET);
    }
    older = opened.data.older;
    // The core keeps the history; the pane keeps the same, so scrolling back shows what the
    // session has rather than what this terminal happens to have seen.
    pane.options.scrollback = opened.data.scrollback;
    newline = opened.data.newline ?? undefined;
    pane.resize(opened.data.columns, opened.data.rows);
    for (const held of waiting.splice(0)) writes.add(held.bytes, held.written);
    // Now that the screen is drawn, the pane's own size applies again.
    fit();
    done();
    paging.opened(history, older);
  };
  void open(FIRST_PAGE);

  live.release = () => {
    gone = true;
    openingDone();
    unfollow();
    unsize();
    unfont();
    unread();
    typed.dispose();
    bytes.dispose();
    paging.dispose();
    resized.dispose();
    if (view !== undefined) void commands.unwatchSession(plane, session, view);
    writes.dispose();
    watched.dispose();
    altScreenKept.dispose();
    links.dispose();
    echo.dispose();
    pane.dispose();
    bench.paneClosed(session, pane);
  };
  return live;
}

/** Screen-reader mode (DS-6, #629): kept in the layout file by `screenReader.ts`, and said
 *  from here too, where every chat's terminal follows it. */
export { screenReaderMode, setScreenReaderMode } from "./screenReader";

/** The find bar's key, read by the key registry too (#2921): `keys/findKeys.ts`. */
export { opensFind };

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
 * Tab walks (`tabSequence.ts`), past a split's resize handle, so it lands on the next pane's
 * head.
 */
export function leavesTheChat(event: KeyboardEvent<HTMLElement>) {
  if (!isKey(event.nativeEvent, "chat.leave")) return;
  event.preventDefault();
  event.stopPropagation();
  moveAlong(event.currentTarget, event.shiftKey, isASplitHandle);
}

/**
 * **A split's resize handle is passed on the way out of a terminal** (#4767, ruled on #4699):
 * Ctrl+Tab out of one pane's terminal lands on the next pane's head, not on the handle between
 * the two panes, which Tab still reaches. A region's handle is no pane's, and stays a stop.
 */
function isASplitHandle(el: HTMLElement): boolean {
  return el.getAttribute("role") === "separator" && el.classList.contains("pane-split");
}
