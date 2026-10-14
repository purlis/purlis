// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useRef, type MouseEvent, type ReactNode } from "react";
import * as ContextMenu from "./overlays/context-menu";
import type { Catalogued, Offer } from "./actions";
import { menuOf, type GroupDrawn } from "./menuModel";
import { Groups } from "./Menus";
import {
  isShiftF10,
  isTerminalRow,
  terminalGroup,
  type TerminalRow,
  type TerminalRowId,
} from "./terminalRows";

/** The pane's own rows, for the terminal's menu to list under its own: the catalogue's offers
 *  a menu on the panes draws (`actions.menuOn`, `pane`), and what carries one out. */
export type PaneMenu = { offers: Catalogued; onPress: (offer: Offer) => void };

/**
 * **The menu a right-click on a terminal opens**: the terminal's own rows (`terminalRows.ts`),
 * then, under a line, the rows the panes' menu has always had there — a new tab, the splits,
 * ending the chat — so nothing a right-click on a pane reached is stranded.
 *
 * **A menu of its own, inside the panes'** (`PlaneView`, `Menued on="pane"`): Radix opens the
 * innermost trigger's and the outer one stands back, as a pane's breadcrumb already does. The
 * panes' menu is still what a right-click on a pane's top line or on a chat still starting
 * opens.
 *
 * **Declared and drawn as every other context menu is** (`docs/ui-primitives.md`, "Menus"):
 * its rows are a `GroupDrawn[]` — the terminal's own group, built here from `terminalRows.ts`,
 * then the groups `menuOf` makes of the panes' rows — drawn by `Menus.tsx`'s `Groups` inside a
 * `ContextMenu.Content` wearing `ui-menu` and named by `aria-label`. So a row's words, the key
 * at its end, why it cannot run, its tooltip and the keyboard are the one menu's. Not modal,
 * for `Menued`'s reason.
 *
 * **Closed on Escape or on one of the terminal's own rows, the keyboard is the terminal's**
 * (`onGiveBack`), said outright: where the engine thinks the keyboard was when the menu opened
 * is not always the terminal. A pane's row decides where the keyboard goes itself (a split's
 * new pane takes it), and a click outside leaves it where the click put it, both as Radix has
 * them.
 *
 * **A second Shift+F10 closes the menu and is the program's** (`onPassKey`): the key that
 * opened it is taken from the program, and this is how it is handed back, as a second `F2`
 * hands `F2` back from the palette. A key held down is one press: its repeats do nothing.
 *
 * **Opened from the keyboard, it rests on no row** ({@link NO_ENTRY_FOCUS}). Radix puts the
 * keyboard on the first row that can run, and with nothing selected Copy cannot, so that row
 * is Paste: the menu key and then Enter would paste, and run the clipboard's lines in a
 * program that did not ask for a bracketed paste. So the keyboard is on the menu itself, and
 * the first arrow, or a letter, moves it to a row. Enter before that does nothing.
 */
/**
 * Radix's own switch for where the keyboard lands when a menu takes it: the menu's roving
 * focus asks before it focuses the first row, and a prevented ask leaves the keyboard on the
 * menu's content. Radix's Menu takes this prop and its ContextMenu hands every prop on to it,
 * but ContextMenu's types leave the name out, which is why it is spread in and not written as
 * a prop. `SessionPane.menu.test.tsx` holds what it is for, so a Radix that stopped honouring
 * it fails there.
 */
const NO_ENTRY_FOCUS: object = { onEntryFocus: (event: Event) => event.preventDefault() };

export function TerminalMenu({
  rows,
  onRun,
  pane,
  onAsked,
  onOpenChange,
  onGiveBack,
  onPassKey,
  children,
}: {
  /** The terminal's rows, as they stood when the menu opened. */
  rows: readonly TerminalRow[];
  onRun: (id: TerminalRowId) => void;
  /** The pane's own rows, where the window has a catalogue to read them from. */
  pane?: PaneMenu;
  /** The `contextmenu` event, before Radix reads it: preventing its default opens no menu
   *  here or on the panes, which is how a click the program asked for stays the program's. */
  onAsked: (event: MouseEvent<HTMLElement>) => void;
  onOpenChange: (open: boolean) => void;
  /** Puts the keyboard in the terminal. */
  onGiveBack: () => void;
  /** Sends the program the Shift+F10 the menu was opened with. */
  onPassKey: () => void;
  /** The element the terminal is drawn in. One element: it is the trigger itself. */
  children: ReactNode;
}) {
  const groups: GroupDrawn[] = [
    terminalGroup(rows),
    ...(pane ? menuOf({ on: "pane" }, pane.offers).groups : []),
  ];
  /** Whether this close gives the keyboard to the terminal, and whether it owes the key. */
  const closing = useRef({ giveBack: false, passKey: false });
  return (
    <ContextMenu.Root modal={false} onOpenChange={onOpenChange}>
      <ContextMenu.Trigger asChild onContextMenu={onAsked}>
        {children}
      </ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Content
          className="ui-menu item-menu"
          collisionPadding={8}
          // What the menu is about, said once, and by no row.
          aria-label="Terminal"
          {...NO_ENTRY_FOCUS}
          onEscapeKeyDown={() => {
            closing.current.giveBack = true;
          }}
          onKeyDown={(event) => {
            if (!isShiftF10(event)) return;
            event.preventDefault();
            // A key held down is the press that opened the menu, still held.
            if (event.repeat) return;
            closing.current = { giveBack: true, passKey: true };
            // Radix closes a menu on Escape and has no other way in from here.
            event.currentTarget.dispatchEvent(
              new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }),
            );
          }}
          onCloseAutoFocus={(event) => {
            const { giveBack, passKey } = closing.current;
            closing.current = { giveBack: false, passKey: false };
            if (!giveBack) return;
            event.preventDefault();
            onGiveBack();
            if (passKey) onPassKey();
          }}
        >
          <Groups
            groups={groups}
            onPress={(offer) => {
              // One of the terminal's own rows, by its id; any other is the pane's.
              if (!isTerminalRow(offer.id)) return pane?.onPress(offer);
              closing.current.giveBack = true;
              onRun(offer.id);
            }}
          />
        </ContextMenu.Content>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  );
}
