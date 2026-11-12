// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { RowMenu } from "./ui/list/List";
import { useEffect, useId, useState, type ReactNode, type RefObject } from "react";
import type { PaneTerminal } from "./paneTerminal";
import { isTerminalRow, terminalGroup, terminalRows } from "./terminalRows";
import { onAMac } from "./mac";
import { Ellipsis } from "lucide-react";
import {
  noteOf,
  PASS_THROUGH_ID,
  renameId,
  type Catalogued,
  type MenuOn,
  type Offer,
} from "./actions";
import { turnStateOf, useChatsHere, useChatsSelect } from "./chatState";
import { menuIsEmpty, menuOf, type GroupDrawn, type RowDrawn, type SubDrawn } from "./menuModel";
import { landSettingsFocus } from "./settings/entering";
import { ContextMenu } from "./ui/menu/ContextMenu";
import { IconButton } from "./ui/button/Button";
import {
  Menu,
  MenuItem,
  MenuSection,
  MenuSeparator,
  MenuTrigger,
  MenuWords,
  Submenu,
} from "./ui/menu/Menu";
import { MenuTip, useRested } from "./ui/menu/MenuTip";
import { ariaKeysOfSaid } from "./keys/registry";

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
 * **The menu family's context menu** (`ui/menu/ContextMenu.tsx`, #2909): the keyboard, the
 * dismissal (a click outside, Escape, a right-click elsewhere), the placement and Shift+F10 and
 * the menu key are the family's, and not a fifth hand-written `ArrowDown`.
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
  /** The element the menu belongs to. One element, because it is the trigger itself — no
   *  wrapper is added to a strip whose layout is measured. */
  children: ReactNode;
}) {
  // Nothing to offer — a project this window holds while its catalogue has not been built yet,
  // for instance. The element is drawn exactly as it was, and `useNoBrowserMenu` still takes
  // the browser's own menu away: a surface with no charter menu must not fall back to
  // `Reload` and `Inspect Element`.
  if (menuIsEmpty(on, offers)) return <>{children}</>;
  return (
    <ContextMenu
      menu={() => <CatalogueMenu on={on} offers={offers} onPress={onPress} />}
      // A row that went into Settings sent the keyboard there, not back to the tab (#1206).
      onClosed={landSettingsFocus}
    >
      {children}
    </ContextMenu>
  );
}

/**
 * **The catalogue's menu for a row a collection draws** (#2911): what `Menued` puts around an
 * element, as a row's `menu` (`ui/list/List`), or none where the catalogue offers nothing.
 */
export function catalogueMenu(
  on: MenuOn,
  offers: Catalogued,
  onPress: (offer: Offer) => void,
): RowMenu | undefined {
  if (menuIsEmpty(on, offers)) return undefined;
  return {
    menu: () => <CatalogueMenu on={on} offers={offers} onPress={onPress} />,
    // A row that went into Settings sent the keyboard there, not back to the row (#1206).
    onClosed: landSettingsFocus,
  };
}

/**
 * **A chat's ⋯** (#4753): its one list, the same `menuOf({ on: "chat" })` its sidebar row's
 * right-click and ⋯ open, from a button of its own, on its pane's head. Named for the chat, so
 * two panes side by side have two names. Nothing is drawn for a chat with no row at all.
 */
export function ChatMenuButton({
  session,
  offers,
  onPress,
}: {
  session: number;
  offers: Catalogued;
  onPress: (offer: Offer) => void;
}) {
  const on: MenuOn = { on: "chat", session };
  if (menuIsEmpty(on, offers)) return null;
  const name = offers.get(renameId(session))?.name;
  const said = moreForSaid(name);
  return (
    // A row that went into Settings sent the keyboard there, not back to the button (#1206).
    <MenuTrigger onClosed={landSettingsFocus}>
      <IconButton
        variant="quiet"
        size="sm"
        aria-label={said}
        tooltip={said}
        icon={<Ellipsis aria-hidden="true" />}
      />
      {/* A pane's head is drawn once per pane, never per tab of a strip, so the model is made
          with it: a handful of scans, not fifty. */}
      <CatalogueMenu on={on} offers={offers} onPress={onPress} />
    </MenuTrigger>
  );
}

/**
 * **A right-click on a chat's pane head** (#4760, spec #4750 story 60): the chat's one list,
 * the same its ⋯ opens, then under a line the pane's own rows (`menuOn({ on: "pane" })`), of
 * this pane and not the focused one. `pane` is those rows as this pane offers them: its close is
 * its own. Inside the panes' menu, so the innermost one opens, as a terminal's does.
 */
export function PaneHeadMenued({
  session,
  offers,
  pane,
  tab,
  terminal,
  onPress,
  children,
}: {
  session: number;
  offers: Catalogued;
  pane: Catalogued;
  /** The tab the pane is in: its Tile all is one of the pane's rows (#4765). */
  tab?: number;
  /** The terminal the pane draws, where it draws one: its Copy and Paste are pane rows. */
  terminal?: RefObject<PaneTerminal | null>;
  onPress: (offer: Offer) => void;
  /** The pane head. One element: it is the trigger itself. */
  children: ReactNode;
}) {
  /** The terminal as the menu opened: whether anything was selected is read then, once. */
  const [terminalNow, setTerminalNow] = useState<PaneTerminal | null>(null);
  const on: MenuOn = { on: "chat", session };
  if (menuIsEmpty(on, offers) && menuIsEmpty({ on: "pane", tab }, pane)) return <>{children}</>;
  return (
    <ContextMenu
      onOpenChange={(open) => setTerminalNow(open ? (terminal?.current ?? null) : null)}
      menu={() => (
        <PaneHeadMenu
          session={session}
          offers={offers}
          pane={pane}
          tab={tab}
          terminal={terminalNow}
          onPress={onPress}
        />
      )}
      onClosed={landSettingsFocus}
    >
      {children}
    </ContextMenu>
  );
}

/**
 * That menu's content, made only while it is open, as `CatalogueMenu`'s is. **The pane's rows
 * in the table's order** (`command-table.md`, "Panes"): the splits and Tile all, then the
 * terminal's Copy and Paste, then Send F2, and under a line Close pane. Select all stays the
 * terminal's own right-click's.
 */
function PaneHeadMenu({
  session,
  offers,
  pane,
  tab,
  terminal,
  onPress,
}: {
  session: number;
  offers: Catalogued;
  pane: Catalogued;
  tab?: number;
  terminal: PaneTerminal | null;
  onPress: (offer: Offer) => void;
}) {
  const chat = menuOf({ on: "chat", session }, offers);
  const [rows, ...below] = menuOf({ on: "pane", tab }, pane).groups;
  const copyPaste =
    terminal === null
      ? []
      : terminalGroup(
          terminalRows({ selected: terminal.selected(), mac: onAMac() }).filter(
            (row) => row.id !== "terminal.selectAll",
          ),
        ).entries;
  const at = (rows?.entries ?? []).findIndex(
    (entry) => entry.kind === "row" && entry.offer.id === PASS_THROUGH_ID,
  );
  const paneRows: GroupDrawn[] =
    rows === undefined
      ? copyPaste.length === 0
        ? []
        : [{ entries: copyPaste }]
      : [
          {
            ...rows,
            entries:
              at === -1
                ? [...rows.entries, ...copyPaste]
                : [...rows.entries.slice(0, at), ...copyPaste, ...rows.entries.slice(at)],
          },
        ];
  return (
    <Menu aria-label={chat.about} className="item-menu">
      <Groups
        groups={[...chat.groups, ...paneRows, ...below]}
        onPress={(offer) => {
          // One of the terminal's own rows, by its id, runs on it; any other is the catalogue's.
          if (isTerminalRow(offer.id)) terminal?.run(offer.id);
          else onPress(offer);
        }}
      />
    </Menu>
  );
}

/** What a chat's ⋯ is called, on its pane and its row (#4753): for the chat it opens the list of. */
export function moreForSaid(name: string | undefined): string {
  return name === undefined ? "More for this chat" : `More for ${name}`;
}

/**
 * The menu's content. A component of its own so the model is made **only while the menu is
 * open**: the context menu draws it only then, and `menuOf` scans the catalogue for the Ask,
 * Curate and Move to rows, which a strip of fifty tabs must not pay per render.
 */
export function CatalogueMenu({
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
    // **The thing the menu is about is said once, here**, and by no row: "Tab steward 4".
    <Menu aria-label={menu.about} className="item-menu">
      <Groups groups={menu.groups} onPress={onPress} />
    </Menu>
  );
}

/**
 * A menu's groups, or a submenu's: a line between two, and a group's name over it.
 *
 * **Exported for a context menu whose rows are not the catalogue's** (a terminal's own menu):
 * it builds its `GroupDrawn[]` itself, each row around an `Offer` made for that row as
 * `actions.fileRows` makes them, and draws them here inside its own `Menu`, so its rows are
 * drawn and behave as every other menu's (`docs/ui-primitives.md`, "Menus").
 *
 * **A toggle is a row of a group that ticks** (`menuitemcheckbox`): the rows of a group are
 * drawn as runs, each toggle's run a group of its own that chooses many and keeps the group's
 * name on its first, so a group with a toggle among its rows still reads as one group.
 */
export function Groups({
  groups,
  onPress,
}: {
  groups: readonly GroupDrawn[];
  onPress: (offer: Offer) => void;
}) {
  return groups.map((group, i) => {
    const key = keyOf(group);
    return [
      i > 0 && <MenuSeparator key={`${key}-line`} />,
      ...runsOf(group).map((run, at) => {
        const toggles = run[0].kind === "row" && run[0].checked !== undefined;
        const ticked = new Set(
          run.flatMap((entry) => (entry.kind === "row" && entry.checked ? [entry.offer.id] : [])),
        );
        return (
          <MenuSection
            key={`${key}-${at}`}
            id={`${key}-${at}`}
            // A name is data and is written as it was given; only purlis's own words wear the
            // label role's capitals (ruling T4).
            title={at === 0 ? group.label : undefined}
            named={group.named}
            {...(toggles
              ? {
                  selectionMode: "multiple" as const,
                  selectedKeys: ticked,
                  // A toggle's press is a row's press, and closes the menu as a row's does.
                  shouldCloseOnSelect: true,
                }
              : {})}
          >
            {run.map((entry) =>
              entry.kind === "row" ? (
                <Row key={entry.offer.id} row={entry} onPress={onPress} />
              ) : (
                <Sub key={entry.key} sub={entry} onPress={onPress} />
              ),
            )}
          </MenuSection>
        );
      }),
    ];
  });
}

/** A group's entries in runs: its toggles apart from its other rows, in their order. */
function runsOf(group: GroupDrawn): GroupDrawn["entries"][] {
  const runs: GroupDrawn["entries"][] = [];
  let toggles: boolean | undefined;
  for (const entry of group.entries) {
    const toggle = entry.kind === "row" && entry.checked !== undefined;
    if (toggle !== toggles) runs.push([]);
    toggles = toggle;
    runs[runs.length - 1].push(entry);
  }
  return runs;
}

function keyOf(group: GroupDrawn): string {
  const first = group.entries[0];
  return (
    group.label ?? (first === undefined ? "" : first.kind === "row" ? first.offer.id : first.key)
  );
}

/**
 * A row that opens a submenu: *Open*, *Ask*, *Stop*, *Curate*, *Move to*. The family's own
 * `Submenu`, so Right opens it, Left and Escape close it, and the hover intent and the
 * placement of a nested menu are React Aria's.
 *
 * What every row inside does is said once, as this row's tooltip and description, and on the
 * side away from the submenu it opens.
 */
function Sub({ sub, onPress }: { sub: SubDrawn; onPress: (offer: Offer) => void }) {
  const noted = useId();
  const { rested, rest, wake, hold } = useRested(sub.tip !== undefined);
  return (
    <Submenu
      aria-label={sub.said}
      className="item-menu"
      row={
        <MenuItem
          id={`sub-${sub.key}`}
          textValue={sub.said}
          aria-describedby={sub.tip === undefined ? undefined : noted}
          {...rest}
        >
          <MenuWords said={sub.said} />
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
        </MenuItem>
      }
    >
      <Groups groups={sub.groups} onPress={onPress} />
    </Submenu>
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
  // Waiting on its own work is mid-turn here too (#3887): a restart waits for that work.
  const state = useChatsSelect(useChatsHere(), (states) =>
    about === undefined ? undefined : turnStateOf(states, about),
  );
  const said = state === "running" && row.midTurn !== undefined ? row.midTurn : row.said;
  const note = noteOf(offer, state === "unknown");
  // Under its words: why it cannot run, or what it costs where that cannot be undone, or else
  // what it acts on (a link's target).
  const under = offer.available ? (row.warns ?? row.target) : offer.reason;
  // Its tooltip: the catalogue's note, for a row that can run and is not one of a submenu that
  // says it once. A row that cannot run says why under its words, and as its description.
  const tip = offer.available && !row.untipped ? note : undefined;
  // Its description: what is under its words (`MenuWords` hands the row that), then the
  // catalogue's note where it says something more, each heard once.
  const described = offer.available && note !== under ? note : undefined;
  const { rested, rest, wake, hold } = useRested(tip !== undefined);
  return (
    <MenuItem
      id={offer.id}
      // `danger` is the look; `ends-it` is the house meaning, "cannot be undone from purlis"
      // (D-1719-3), which a migrating control keeps beside its look (DS-2).
      variant={row.danger ? "danger" : "default"}
      className={row.danger ? "ends-it" : undefined}
      keys={row.keys}
      // The key as a reader hears it, where no registry entry answers for the row (#6043).
      aria-keyshortcuts={row.keys === undefined ? undefined : ariaKeysOfSaid(row.keys)}
      shortcut={offer.id}
      isDisabled={!offer.available}
      // **The row's NAME is its words and nothing else**, and its note is its description. Its
      // content names it (#6043): what stands under its words and its key are kept out of the
      // name and handed to it as its description and its `aria-keyshortcuts`, so the name is
      // its only visible words (WCAG 2.5.3), and no row announces as a paragraph. Never an
      // `aria-label`, which would leave out the visible words under it. Type-ahead reads the
      // same words.
      textValue={said}
      aria-describedby={described ? noted : undefined}
      onAction={() => {
        wake();
        onPress(offer);
      }}
      {...rest}
    >
      <MenuWords said={said} under={under} />
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
    </MenuItem>
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
 * **On the bubble phase, and that is load-bearing.** The family's context menu opens from an
 * `onContextMenu` that stands back when the event is already `defaultPrevented` (a menu inside
 * it answered). React 19 attaches its delegated listeners to the root container, which is
 * below `window`, so a CAPTURING listener here would run first, prevent the default, and
 * silently stop every menu in this file from ever opening. On the bubble phase the menu has
 * already opened — and already prevented the default itself — and this only covers what it
 * did not.
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
