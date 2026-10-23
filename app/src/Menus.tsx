// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useId, type KeyboardEvent, type ReactNode } from "react";
import * as ContextMenu from "./overlays/context-menu";
import { Check, ChevronRight } from "lucide-react";
import { noteOf, type Catalogued, type MenuOn, type Offer } from "./actions";
import { stateOf, useChatsHere, useChatsSelect } from "./chatState";
import { menuIsEmpty, menuOf, type GroupDrawn, type RowDrawn, type SubDrawn } from "./menuModel";
import { MenuKeys, MenuTip, MenuWords, useRested } from "./primitives/MenuRow";
import { landSettingsFocus } from "./settings/entering";

/**
 * The window's context menus: right-click on a thing, and charter offers what it can do to it.
 *
 * **A menu is a third reader of `actions.ts`, and never a list of its own.** The palette reads
 * the catalogue and so does the bar; `actions.menuRows` filters the same catalogue to the item
 * the menu was opened on, and everything drawn here — the words, whether a row can run, the
 * reason it cannot, what it does — is the offer that came back. Nothing in this file knows what
 * any action means. That is the rule `actions.ts` opens with, and a menu is exactly the surface
 * the tmux frame grew a second answer on: `frame/tabmenu.py` and the palette drifted, and the
 * fix there was to make the menu a view of the one catalogue rather than to keep them in step.
 *
 * **How a row is drawn is `menuModel.ts`'s, for every menu alike**: its short words, its group,
 * the submenu it is folded into, its key, and whether it is in the danger colour. This file
 * draws that model and decides none of it (`docs/ui-primitives.md`, "Menus").
 *
 * **What cannot be undone is last, under a line, and marked.** The catalogue puts such rows last
 * so that Enter in the palette is never one keystroke from ending a chat. A context menu pops
 * up under the pointer, where the operator did not choose the layout — so the line and the
 * danger colour are the same rule with more at stake, not less.
 *
 * **Not modal** (`modal={false}`), for the show-more menu's reason (`docs/ui-primitives.md`): a
 * modal Radix surface marks the rest of the window `aria-hidden`, which is right for a question
 * that must be answered and wrong for a menu on a strip. A click outside closes it, which is
 * what every menu on every platform does, and there is no answer to lose.
 *
 * Radix's `ContextMenu` (ADR 0037), so the keyboard, the roving focus, the dismissal and the
 * collision handling are the primitive's and not a fifth hand-written `ArrowDown`.
 */
export function Menued({
  on,
  offers,
  onPress,
  children,
}: {
  /** The item this menu is about. */
  on: MenuOn;
  /** The catalogue as it stands, by id — the same rows the palette lists and the bar reads,
   *  indexed once for the whole window. A menu on a strip is drawn per tab per render, so an
   *  array here would be a scan per tab per render over a list that #174 made half as long
   *  again; `actions.catalogued` has what that measured and why it was changed. */
  offers: Catalogued;
  onPress: (offer: Offer) => void;
  /** The element the menu belongs to. One element, because it is the trigger itself —
   *  `asChild`, so no wrapper is added to a strip whose layout is measured. */
  children: ReactNode;
}) {
  // Nothing to offer — a project this window holds while its catalogue has not been built yet,
  // for instance. The element is drawn exactly as it was, and `useNoBrowserMenu` still takes
  // the browser's own menu away: a surface with no charter menu must not fall back to
  // `Reload` and `Inspect Element`.
  if (menuIsEmpty(on, offers)) return <>{children}</>;
  return (
    <ContextMenu.Root modal={false}>
      <ContextMenu.Trigger asChild onKeyDown={openFromTheKeyboard}>
        {children}
      </ContextMenu.Trigger>
      <ContextMenu.Portal>
        <Drawn on={on} offers={offers} onPress={onPress} />
      </ContextMenu.Portal>
    </ContextMenu.Root>
  );
}

/**
 * The menu's content. A component of its own so the model is made **only while the menu is
 * open**: Radix mounts what is in the portal only then, and `menuOf` scans the catalogue for
 * the Ask, Curate and Move to rows, which a strip of fifty tabs must not pay per render.
 */
function Drawn({
  on,
  offers,
  onPress,
}: {
  on: MenuOn;
  offers: Catalogued;
  onPress: (offer: Offer) => void;
}) {
  const menu = menuOf(on, offers);
  return (
    <ContextMenu.Content
      className="ui-menu item-menu"
      collisionPadding={8}
      // **The thing the menu is about is said once, here**, and by no row: "Tab steward 4".
      aria-label={menu.about}
      onCloseAutoFocus={(event) => {
        // A row that went into Settings sent the keyboard there, not back to the tab (#1206).
        if (landSettingsFocus()) event.preventDefault();
      }}
    >
      <Groups groups={menu.groups} onPress={onPress} />
    </ContextMenu.Content>
  );
}

/**
 * A menu's groups, or a submenu's: a line between two, and a group's name over it.
 *
 * **Exported for a context menu whose rows are not the catalogue's** (a terminal's own menu):
 * it builds its `GroupDrawn[]` itself, each row around an `Offer` made for that row as
 * `actions.fileRows` makes them, and draws them here inside its own `ContextMenu.Content`, so
 * its rows are drawn and behave as every other menu's (`docs/ui-primitives.md`, "Menus").
 */
export function Groups({
  groups,
  onPress,
}: {
  groups: readonly GroupDrawn[];
  onPress: (offer: Offer) => void;
}) {
  return groups.map((group, i) => (
    // Keyed by its first row: a group has no name of its own, and its place can change.
    <ContextMenu.Group key={keyOf(group)}>
      {i > 0 && <ContextMenu.Separator className="ui-menu-line" />}
      {group.label !== undefined && (
        // A name is data and is written as it was given; only purlis's own words wear the
        // label role's capitals (ruling T4).
        <ContextMenu.Label
          className={
            group.named
              ? "ui-menu-label ui-menu-label-named ui-text-secondary"
              : "ui-menu-label ui-text-label"
          }
        >
          {group.label}
        </ContextMenu.Label>
      )}
      {group.entries.map((entry) =>
        entry.kind === "row" ? (
          <Row key={entry.offer.id} row={entry} onPress={onPress} />
        ) : (
          <Sub key={entry.key} sub={entry} onPress={onPress} />
        ),
      )}
    </ContextMenu.Group>
  ));
}

function keyOf(group: GroupDrawn): string {
  const first = group.entries[0];
  return (
    group.label ?? (first === undefined ? "" : first.kind === "row" ? first.offer.id : first.key)
  );
}

/**
 * A row that opens a submenu: *Open*, *Ask*, *Stop*, *Curate*, *Move to*. Radix's own
 * `ContextMenu.Sub` (ADR 0037), so Right opens it, Left and Escape close it, and the hover
 * intent and the collision handling of a nested menu are the primitive's.
 *
 * What every row inside does is said once, as this row's tooltip and description, and on the
 * side away from the submenu it opens.
 */
function Sub({ sub, onPress }: { sub: SubDrawn; onPress: (offer: Offer) => void }) {
  const noted = useId();
  const { rested, rest, wake, hold } = useRested(sub.tip !== undefined);
  return (
    <ContextMenu.Sub>
      <ContextMenu.SubTrigger
        className="ui-menu-item"
        textValue={sub.said}
        aria-label={sub.said}
        aria-describedby={sub.tip === undefined ? undefined : noted}
        {...rest}
      >
        <MenuWords said={sub.said} />
        <span className="ui-menu-mark" aria-hidden="true">
          <ChevronRight />
        </span>
        {sub.tip !== undefined && (
          <span id={noted} hidden>
            {sub.tip}
          </span>
        )}
        {rested && (
          <MenuTip side="left" onClose={wake} hold={hold}>
            {sub.tip}
          </MenuTip>
        )}
      </ContextMenu.SubTrigger>
      <ContextMenu.Portal>
        <ContextMenu.SubContent className="ui-menu item-menu" collisionPadding={8}>
          <Groups groups={sub.groups} onPress={onPress} />
        </ContextMenu.SubContent>
      </ContextMenu.Portal>
    </ContextMenu.Sub>
  );
}

/**
 * Opens the menu from Shift+F10 or the menu key, on the element that has the keyboard
 * (charter-app#174).
 *
 * **Whether the engine raises `contextmenu` for either key is the platform's, not charter's**
 * — macOS has no such convention at all, so in the macOS WebView every menu in the window was
 * a pointer's alone, including the explorer's rows, which the keyboard otherwise reaches only
 * by arrows. This sends the event a right-click sends, at the element's lower-left corner, so
 * Radix opens exactly the menu a pointer would and nothing here decides what is in it. The
 * default is prevented; an engine that raises its own event as well only opens the same menu
 * again. Radix already knows the keyboard is in use (the key was a keydown), so it puts the
 * first row under the keyboard.
 *
 * **Only when the trigger ITSELF has the keyboard.** The panes' menu is on the box the
 * terminals are in, and a program in a terminal may want Shift+F10; a key pressed inside a
 * trigger belongs to what it was pressed in. A terminal answers the two keys itself, with its
 * own menu (`SessionPane`, `terminalRows.asksForTheMenu`).
 */
function openFromTheKeyboard(event: KeyboardEvent<HTMLElement>) {
  if (event.target !== event.currentTarget) return;
  if (event.key !== "ContextMenu" && !(event.key === "F10" && event.shiftKey)) return;
  event.preventDefault();
  const at = event.currentTarget.getBoundingClientRect();
  event.currentTarget.dispatchEvent(
    new MouseEvent("contextmenu", {
      bubbles: true,
      cancelable: true,
      clientX: at.left,
      clientY: at.bottom,
    }),
  );
}

/** One row of a menu: a catalogue offer, drawn as the model says.
 *
 *  **Its note is its tooltip and its description, never a line of its own.** What stands under
 *  a row's words is one of two things, and is never cut: what a row that cannot be undone
 *  costs, or why a row cannot run. A row that cannot run is drawn with its reason rather than left out,
 *  which is the catalogue's own rule: an operator cannot ask about an option they cannot see. */
function Row({ row, onPress }: { row: RowDrawn; onPress: (offer: Offer) => void }) {
  const { offer } = row;
  const noted = useId();
  // A row about a chat that is mid-turn says so, and one about a chat that reports no state
  // says that, both read as the menu is drawn (`Offer.midTurn`, `Offer.noState`).
  const about = offer.midTurn?.session ?? offer.noState?.session;
  const state = useChatsSelect(useChatsHere(), (states) =>
    about === undefined ? undefined : stateOf(states, about),
  );
  const said = state === "running" && row.midTurn !== undefined ? row.midTurn : row.said;
  const note = noteOf(offer, state === "unknown");
  // Under its words: why it cannot run, or what it costs where that cannot be undone.
  const under = offer.available ? row.warns : offer.reason;
  // Its tooltip: the catalogue's note, for a row that can run and is not one of a submenu that
  // says it once. A row that cannot run says why under its words, and as its description.
  const tip = offer.available && !row.untipped ? note : undefined;
  const described = offer.available ? (note ?? row.warns) : offer.reason;
  const { rested, rest, wake, hold } = useRested(tip !== undefined);
  const inside = (
    <>
      <MenuWords said={said} under={under} danger={row.danger} />
      {row.keys !== undefined && <MenuKeys>{row.keys}</MenuKeys>}
      {described !== undefined && described !== "" && (
        <span id={noted} hidden>
          {described}
        </span>
      )}
      {rested && (
        <MenuTip side="right" onClose={wake} hold={hold}>
          {tip}
        </MenuTip>
      )}
    </>
  );
  const shared = {
    disabled: !offer.available,
    // **The row's NAME is its words and nothing else**, and its note is its description. Left
    // to the content, an accessible name would be the words with the consequence run onto the
    // end of them, so every row would announce as a paragraph and no surface could ask for one
    // by name. Type-ahead reads the same words.
    textValue: said,
    "aria-label": said,
    "aria-describedby": described ? noted : undefined,
    onSelect: () => {
      wake();
      onPress(offer);
    },
    ...rest,
  };
  // A toggle is one row with a tick while it is on, not two rows that take turns.
  if (row.checked !== undefined)
    return (
      <ContextMenu.CheckboxItem className="ui-menu-item" checked={row.checked} {...shared}>
        {inside}
        <ContextMenu.ItemIndicator className="ui-menu-mark">
          <Check className="node-icon" aria-hidden />
        </ContextMenu.ItemIndicator>
      </ContextMenu.CheckboxItem>
    );
  return (
    <ContextMenu.Item
      // `ui-menu-item-danger` is the look; `ends-it` is the house meaning, "cannot be undone
      // from purlis" (D-1719-3), which a migrating control keeps beside its look (DS-2).
      className={row.danger ? "ui-menu-item ui-menu-item-danger ends-it" : "ui-menu-item"}
      {...shared}
    >
      {inside}
    </ContextMenu.Item>
  );
}

/**
 * Takes the WebView's own context menu away from the whole window.
 *
 * **A shipped app that answers a right-click with `Reload` and `Inspect Element` is showing the
 * operator the browser it is built on.** charter's menus above are the answer where there is
 * one; this is the answer everywhere else, and it is one listener rather than an
 * `onContextMenu` on every element, because the thing being removed is a default and a default
 * is not per-element.
 *
 * **On the bubble phase, and that is load-bearing.** Radix opens its menu from an
 * `onContextMenu` composed with `composeEventHandlers`, which skips its own handler when the
 * event is already `defaultPrevented`. React 19 attaches its delegated listeners to the root
 * container, which is below `window`, so a CAPTURING listener here would run first, prevent the
 * default, and silently stop every menu in this file from ever opening. On the bubble phase
 * Radix has already opened — and already prevented the default itself — and this only covers
 * what it did not.
 *
 * **Except where the operator is editing text.** An `input`, a `textarea` or a contenteditable
 * answers a right-click with the platform's own Cut/Copy/Paste, and that menu is not the
 * browser showing through — it is the only pointer route to the clipboard this window has.
 * Taking it away to hide `Inspect Element` would cost paste in the new-workspace box and in the
 * palette to save a line an operator never reads in a release build, where the WebView's
 * inspector is off. Deliberate, and narrow: it is the editing menu that survives, on the
 * element being edited, and nothing else.
 */
export function useNoBrowserMenu() {
  useEffect(() => {
    const suppress = (event: MouseEvent) => {
      if (editing(event.target)) return;
      event.preventDefault();
    };
    window.addEventListener("contextmenu", suppress);
    return () => window.removeEventListener("contextmenu", suppress);
  }, []);
}

/** Whether the right-click landed on something the operator types into. */
function editing(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  const tag = target.tagName;
  return tag === "INPUT" || tag === "TEXTAREA";
}
