// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  askRows,
  curateRows,
  curateSubjectOf,
  menuRows,
  moveRows,
  OUTSIDE_TITLE,
  REVEAL_SAID,
  type Catalogued,
  type MenuOn,
  type Offer,
} from "./actions";
import { cardRowsFor } from "./cardLink";
import { storeLabel } from "./memoryMoves";
import { SAID, SUBMENU, WARNS } from "./menuWords";
import { shellKeySaid } from "./shellKey";
import { deleteKeySaid, onAMac, RENAME_KEY_SAID } from "./tabKeys";

/**
 * **How a context menu is drawn: the one place its rows are named, grouped and marked**
 * (`docs/ui-primitives.md`, "Menus").
 *
 * WHICH rows a menu has is `actions.menuRows`'s answer and is not decided here: this takes those
 * rows, every one of them, and says how each is drawn. So an action cannot be lost or gained by a
 * change to this file, and `menuModel.test.ts` holds that the rows drawn are the rows listed.
 *
 * What it decides, for every menu alike:
 *
 * 1. **A row is a short verb.** The menu is about the thing that was right-clicked, so a row does
 *    not say its name again. The menu says it once, as its own accessible name ({@link
 *    MenuDrawn.about}). A row keeps a name only when it is another thing's: *Go to steward 4*,
 *    *Merge into svc*.
 * 2. **A row's note is its tooltip and its description**, not a line under it. Two kinds of line
 *    stay under a row, and neither is ever cut: what a row that cannot be undone costs, and why
 *    a row cannot run.
 * 3. **Rows of one kind are one row that opens a submenu**: *Open*, *Ask*, *Stop*, *Curate*,
 *    *Move to*.
 * 4. **The danger colour is for what cannot be undone**: a program ended, a thing deleted.
 * 5. **An ellipsis is the catalogue's**: a row has one exactly when its title does. A row that
 *    could only say what a press would do, such as *Switch to* on the tab in front, is left out.
 * 6. **A shortcut is said at the row's end**, and a toggle is one row with a tick.
 * 7. **At most four groups**, in this order: the thing itself, what it opens or starts, how it
 *    runs, and last what ends or deletes it.
 */

/** The four groups of a menu, in the order they are drawn. */
const SLOTS = ["first", "open", "run", "end"] as const;
type Slot = (typeof SLOTS)[number];

/** The submenus rows are folded into. */
type SubKey = keyof typeof SUBMENU;

/** One row of a menu, as it is drawn. */
export type RowDrawn = {
  kind: "row";
  /** The catalogue's row: what a press hands back, whether it can run, its reason and note. */
  offer: Offer;
  /** What the row says. Its accessible name too. */
  said: string;
  /** What it says while the chat it is about is mid-turn (`Offer.midTurn`). */
  midTurn?: string;
  /** Whether it cannot be undone, and is drawn in the danger colour. */
  danger: boolean;
  /** Under a row that cannot be undone: what it costs, short and never cut. */
  warns?: string;
  /** A toggle's state: drawn with a tick while it is on. Not a toggle when absent. */
  checked?: boolean;
  /** The key that does the same from the thing itself, said at the row's end. */
  keys?: string;
  /** A row of a submenu whose own tooltip already says what every row of it does. */
  untipped?: true;
};

/** A row that opens a submenu. */
export type SubDrawn = {
  kind: "sub";
  key: SubKey;
  said: string;
  /** What every row inside does, said once as this row's tooltip. */
  tip?: string;
  groups: GroupDrawn[];
};

export type EntryDrawn = RowDrawn | SubDrawn;

/** Rows drawn together, under a line that parts them from the group before. */
export type GroupDrawn = {
  /** The group's name, over it. Not a row. */
  label?: string;
  /**
   * Whether the name is **data**: a persona's name, written as the person wrote it. A label in
   * purlis's own fixed words wears the label role's capitals; a name never does (ruling T4:
   * labels are uppercase, data never is).
   */
  named?: true;
  entries: EntryDrawn[];
};

/** A whole menu. */
export type MenuDrawn = {
  /** What the menu is about, said once: the menu's accessible name. */
  about: string | undefined;
  /** The name of the thing it was opened on, which no row of it says again. */
  name: string | undefined;
  groups: GroupDrawn[];
};

/** How one catalogue row is drawn in a menu. */
type Dress = {
  /** Its words: written once, or made from the row where they change with it. */
  said: string | ((offer: Offer) => string);
  midTurn?: string;
  /** The group it is drawn in: one, or the one the row itself says where what it does varies. */
  slot: Slot | ((offer: Offer) => Slot);
  /** The submenu it is folded into. */
  under?: SubKey;
  /** Whether it cannot be undone. */
  danger?: boolean | ((offer: Offer) => boolean);
  warns?: string | ((offer: Offer) => string | undefined);
  /** A toggle: whether it is on now. */
  checked?: (offer: Offer) => boolean;
  keys?: (offer: Offer) => string | undefined;
  /** Left out while it cannot run: it would say only what a press does. */
  quiet?: true;
};

/** The kinds of thing a row's title names before the name itself. */
const KINDS = "chat|tab|task|workspace|project|branch|folder|repo|vault|persona|todo|memory";

/**
 * A row's title with the name it carries taken out: *End chat 3 steward* is *End*, *Send task 2
 * back into steward 4's tab* is *Send back into steward 4's tab*. For the rows whose title
 * changes with what they would do, so their words here cannot fall behind the catalogue's.
 *
 * The name is taken out where the title says it: after the word for its kind, or else where it
 * first stands after the title's verb. A chat a person named *End* is still ended by *End*.
 */
export function unnamed(offer: Offer): string {
  const { title, name } = offer;
  if (name === undefined || name === "") return title;
  const kinded = new RegExp(`\\b(?:${KINDS}) ${name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`);
  const at = title.indexOf(name, Math.max(title.indexOf(" "), 0));
  const without = kinded.test(title)
    ? title.replace(kinded, "")
    : at === -1
      ? title
      : title.slice(0, at) + title.slice(at + name.length);
  return without
    .replace(/\s+/g, " ")
    .replace(/ (…|,|:)/g, "$1")
    .trim();
}

/** Whether a pin row's thing is pinned now: the row would unpin it. */
function pinnedNow(offer: Offer): boolean {
  const does = offer.does;
  return (
    (does.verb === "pinTab" || does.verb === "pinWorkspace" || does.verb === "pinProject") &&
    !does.pinned
  );
}

/** Whether a close row ends a chat, as the catalogue says. */
function endsAChat(offer: Offer): boolean {
  const does = offer.does;
  if (does.verb === "closeTab" || does.verb === "closePane") return does.ends;
  // A row that cannot run carries no verb; its title still says which it is.
  return does.verb === "nothing" && offer.title.startsWith("End ");
}

const pin: Dress = { said: SAID.pinned, slot: "first", checked: pinnedNow };
const stopped = (warns: string, said: Dress["said"] = unnamed): Dress => ({
  said,
  // Each way to stop ends a program, so the row that opens them is in the last group.
  slot: "end",
  under: "stop",
  danger: true,
  warns,
});

/**
 * **Every row a menu can list, by the part of its id before the colon, and how it is drawn.**
 * A row that is not here is drawn by its title, where `actions.menuOn` put it, so a new row is
 * never lost; `menuModel.test.ts` lists the ones that are meant to be.
 */
const DRESS: Readonly<Record<string, Dress>> = {
  // A chat's tab, and a chat's row in the Chats list.
  "tab.smartclose.cancel": { said: SAID.cancelSmartClose, slot: "first" },
  "tab.select": { said: SAID.switchTo, slot: "first", quiet: true },
  "tab.rename": { said: SAID.rename, slot: "first", keys: () => RENAME_KEY_SAID },
  "tab.pin": pin,
  "tab.worklink": { said: SAID.linkWorkItem, slot: "first" },
  "tab.workunlink": { said: SAID.unlinkWorkItem, slot: "first" },
  "needs.show": { said: SAID.show, slot: "first" },
  "chat.handed": { said: (offer) => `Go to ${offer.name ?? ""}`.trim(), slot: "first" },
  "chat.own": { said: SAID.ownTab, slot: "open" },
  "chat.beside": { said: unnamed, slot: "open" },
  "chat.back": { said: unnamed, slot: "open" },
  "chat.brief": { said: SAID.brief, slot: "open" },
  "chat.answer": { said: SAID.answer, slot: "open" },
  "tab.activity": { said: SAID.activity, slot: "open", under: "open" },
  "tab.network": { said: SAID.network, slot: "open", under: "open" },
  // The row's own words: Use skill…, and the chat's budget where it is known (#2775).
  "tab.useskill": { said: (offer) => offer.title, slot: "open" },
  "tab.restart": { said: SAID.restart, midTurn: SAID.restartMidTurn, slot: "run" },
  "chat.restart": { said: SAID.restart, midTurn: SAID.restartMidTurn, slot: "run" },
  "chat.stop": stopped(WARNS.stops),
  "chat.stop.below": stopped(WARNS.stopsBelow),
  "chat.stop.tasks": stopped(WARNS.stopsTasks, SAID.stopAllTasks),
  "task.stop": { said: SAID.taskStop, slot: "end", danger: true, warns: WARNS.taskStops },
  "task.close": { said: SAID.taskClose, slot: "end", danger: true, warns: WARNS.taskCloses },
  "tab.fresh": { said: SAID.startFresh, slot: "end", danger: true, warns: WARNS.fresh },
  "tab.close": {
    said: unnamed,
    // A task's own tab keeps the close's id for its minimise (#1489), which sends it back and
    // ends nothing: with the other rows that say where a task is drawn, never among what ends.
    slot: (offer) => (offer.does.verb === "sendBack" ? "open" : "end"),
    danger: endsAChat,
    warns: (offer) => (endsAChat(offer) ? WARNS.ends : undefined),
    keys: () => deleteKeySaid(onAMac()),
  },

  // A workspace's tab, and the plane root's.
  "workspace.focus": { said: SAID.focus, slot: "first", quiet: true },
  "workspace.pin": pin,
  "workspace.settings": { said: SAID.settings, slot: "first" },
  "workspace.live": { said: unnamed, slot: "first" },
  "workspace.rename": { said: SAID.rename, slot: "first" },
  "root.chat": { said: SAID.newChat, slot: "open" },
  "shell.new": {
    said: SAID.newShell,
    slot: "open",
    // The key opens a shell where a new tab would start: the row with no place of its own.
    keys: (offer) => (offer.id === "shell.new" ? shellKeySaid(onAMac()) : undefined),
  },
  "workspace.create": { said: (offer) => offer.title, slot: "open" },
  "workspace.remove": {
    said: SAID.delete,
    slot: "end",
    danger: true,
    warns: WARNS.deletesWorkspace,
  },

  // A project's tab.
  "project.select": { said: SAID.switchTo, slot: "first", quiet: true },
  "project.pin": pin,
  "project.settings": { said: SAID.settings, slot: "first" },
  "project.saving": { said: SAID.saving, slot: "first" },
  "project.window": { said: SAID.toNewWindow, slot: "first" },
  "project.main": { said: SAID.toMainWindow, slot: "first" },
  "project.create": { said: (offer) => offer.title, slot: "open" },
  "project.open": { said: (offer) => offer.title, slot: "open" },
  "project.close": {
    said: SAID.close,
    slot: "end",
    danger: true,
    warns: WARNS.closesProject,
    keys: () => deleteKeySaid(onAMac()),
  },

  // A branch's folder, a clone, and a repo that is not cloned here.
  "worktree.focus": { said: SAID.focus, slot: "first" },
  "worktree.files": { said: SAID.browseFiles, slot: "first" },
  "worktree.copypath": { said: SAID.copyAbsolutePath, slot: "open" },
  "worktree.reveal": { said: REVEAL_SAID, slot: "open" },
  "worktree.shell": { said: SAID.shellHere, slot: "open" },
  "worktree.merge": {
    said: (offer) =>
      offer.does.verb === "mergeWorktree" ? `Merge into ${offer.does.cut.repo}` : unnamed(offer),
    slot: "run",
  },
  // The folder goes and the branch stays, and a folder with work in it is refused: no danger.
  "worktree.remove": { said: SAID.removeFolder, slot: "end" },
  "worktree.discard": { said: SAID.discardAnyway, slot: "end", danger: true },
  "clone.focus": { said: SAID.focus, slot: "first" },
  "clone.chat": { said: SAID.newTab, slot: "open" },
  "clone.branch": { said: SAID.newBranch, slot: "open" },
  "clone.pick": { said: SAID.startChatsHere, slot: "run" },
  "absent.clone": { said: SAID.clone, slot: "first" },
  // The workspace stops naming it and nothing is deleted: no danger.
  "absent.drop": { said: SAID.dropFromWorkspace, slot: "end" },
  // Only this window's row goes: no danger.
  "absent.forget": { said: SAID.dismiss, slot: "end" },

  // A persona, a vault, a todo, a session record, a memory.
  "persona.show": { said: SAID.open, slot: "first" },
  "persona.edit": { said: SAID.editDefinition, slot: "first" },
  "persona.profile": { said: SAID.setProfile, slot: "first" },
  "persona.create": { said: (offer) => offer.title, slot: "open" },
  "persona.remove": {
    said: SAID.deleteAsks,
    slot: "end",
    danger: true,
    warns: WARNS.deletesPersona,
  },
  "vault.open": { said: SAID.open, slot: "first" },
  "vault.create": { said: (offer) => offer.title, slot: "open" },
  "vault.remove": { said: SAID.deleteAsks, slot: "end", danger: true, warns: WARNS.deletesVault },
  "todo.open": { said: SAID.open, slot: "first" },
  "todo.done": { said: SAID.markDone, slot: "first" },
  "todo.forget": { said: SAID.forget, slot: "end", danger: true, warns: WARNS.forgets },
  "todo.reopen": { said: SAID.reopen, slot: "first" },
  "todo.delete": { said: SAID.delete, slot: "end", danger: true, warns: WARNS.deletesDone },
  "session.open": { said: SAID.open, slot: "first" },
  "session.resume": { said: SAID.resume, slot: "first" },
  "memory.open": { said: SAID.open, slot: "first" },
  "memory.edit": { said: SAID.edit, slot: "first" },
  // An archive with an Undo (ADR 0065): under the line, as Q12 rules, and not in the danger colour.
  "memory.delete": { said: SAID.delete, slot: "end" },

  // A read of the workspace that stands refused: its one row, on a clone's menu and its own.
  "workspace.readagain": { said: (offer) => offer.title, slot: "run" },

  // A finished task's row. Each asks first, and Discard removes the branch's folder.
  "task.merge": { said: (offer) => offer.title, slot: "first" },
  "task.discard": { said: (offer) => offer.title, slot: "end", danger: true },

  // A file's or a folder's row. No row names it, so the catalogue's words are the row's.
  "file.copy": { said: (offer) => offer.title, slot: "first" },
  "file.copyabsolute": { said: (offer) => offer.title, slot: "first" },
  "file.reveal": { said: (offer) => offer.title, slot: "first" },
  "file.editor": { said: (offer) => offer.title, slot: "open" },
  "file.shell": { said: (offer) => offer.title, slot: "open" },
  "file.chat": { said: (offer) => offer.title, slot: "run" },
  "file.addtochat": { said: (offer) => offer.title, slot: "run" },

  // The panes. Not about one named thing, so the catalogue's words are the row's: what starts
  // in a pane, then what is done to the pane with the keyboard, then what closes it.
  "chat.new": { said: (offer) => offer.title, slot: "open" },
  "pane.split.right": { said: (offer) => offer.title, slot: "run" },
  "pane.split.down": { said: (offer) => offer.title, slot: "run" },
  "pane.sendkey": { said: (offer) => offer.title, slot: "run" },
  "pane.close": {
    said: (offer) => offer.title,
    slot: "end",
    danger: endsAChat,
    warns: (offer) => (endsAChat(offer) ? WARNS.ends : undefined),
  },
};

/** The acts this file has words for. */
export const DRESSED: ReadonlySet<string> = new Set(Object.keys(DRESS));

/** The part of an id that names the act: what is before its first colon. */
export function stemOf(id: string): string {
  const at = id.indexOf(":");
  return at === -1 ? id : id.slice(0, at);
}

/** The rows whose words name ANOTHER thing than the one the menu was opened on, and so may
 *  carry a name: a chat gone to, a chat opened beside, a repo merged into. */
export const NAMES_ANOTHER: ReadonlySet<string> = new Set([
  "chat.handed",
  "chat.beside",
  "chat.back",
  "tab.close",
  "worktree.merge",
]);

function drawnRow(offer: Offer, dress: Dress | undefined, below: boolean): RowDrawn {
  // A row this file does not name: drawn by its title, and as one that loses something when
  // the catalogue listed it below the line.
  if (dress === undefined) return { kind: "row", offer, said: offer.title, danger: below };
  const danger = typeof dress.danger === "function" ? dress.danger(offer) : dress.danger === true;
  const warns = typeof dress.warns === "function" ? dress.warns(offer) : dress.warns;
  return {
    kind: "row",
    offer,
    said: withItsEllipsis(typeof dress.said === "function" ? dress.said(offer) : dress.said, offer),
    midTurn: offer.midTurn === undefined ? undefined : dress.midTurn,
    danger,
    warns: danger && offer.available ? warns : undefined,
    checked: dress.checked?.(offer),
    keys: dress.keys?.(offer),
  };
}

/** **An ellipsis is the catalogue's** (`docs/ui-copy.md`: "asks something first"): a row's
 *  words end with one exactly when its title does. */
function withItsEllipsis(said: string, offer: Offer): string {
  const asks = offer.title.endsWith("…");
  const bare = said.replace(/…$/, "");
  return asks ? `${bare}…` : bare;
}

/** A curation row's words inside its submenu: the action alone. The palette's title names the
 *  subject too (`Curate smart-ide: Safe remove`), and the menu is already about it. */
function curationSaid(offer: Offer): string {
  const said = `Curate ${offer.name ?? ""}: `;
  return offer.title.startsWith(said) ? offer.title.slice(said.length) : offer.title;
}

/** A row of a submenu the catalogue decides the length of, by the words its submenu gives it. */
const plain = (offer: Offer, said: string, untipped?: true): RowDrawn => ({
  kind: "row",
  offer,
  said: withItsEllipsis(said, offer),
  danger: false,
  untipped,
});

/**
 * The "Curate ▸" submenu of a workspace, a persona or the plane (ADR 0061): purlis's own
 * actions, then a named group per persona that declared one, then every action the core left
 * out as a row that cannot run, then the card an off row's reason ends with as a row of its own
 * (#1134: a disabled row is skipped by the keyboard, so a link inside it would be reached by
 * none). Nothing for a subject with no action at all.
 */
function curateSub(subject: string, offers: Catalogued): SubDrawn | undefined {
  const { charter, personas, leftOut } = curateRows(subject, offers);
  if (charter.length === 0 && personas.length === 0 && leftOut.length === 0) return undefined;
  const cards = cardRowsFor(
    [...charter, ...personas.flatMap((group) => group.rows), ...leftOut],
    offers,
  );
  const rows = (some: readonly Offer[]) => some.map((offer) => plain(offer, curationSaid(offer)));
  const groups: GroupDrawn[] = [
    { entries: rows(charter) },
    ...personas.map(({ persona, rows: its }) => ({
      label: persona,
      named: true as const,
      entries: rows(its),
    })),
    { entries: rows(leftOut) },
    { entries: cards.map((offer) => plain(offer, offer.title)) },
  ];
  return {
    kind: "sub",
    key: "curate",
    said: SUBMENU.curate.said,
    tip: SUBMENU.curate.tip,
    groups: groups.filter((group) => group.entries.length > 0),
  };
}

/** What the thing a menu was opened on is called, with its name: the menu's accessible name. */
function aboutOf(what: MenuOn, name: string | undefined): string | undefined {
  const kind: Partial<Record<MenuOn["on"], string>> = {
    chat: "Tab",
    listed: "Chat",
    workspace: "Workspace",
    project: "Project",
    persona: "Persona",
    vault: "Vault",
    todo: "Todo",
    session: "Session record",
    memory: "Memory",
    clone: "Repo",
    absent: "Repo",
  };
  if (what.on === "root") return OUTSIDE_TITLE;
  if (what.on === "pane") return "Panes";
  if (name === undefined) return undefined;
  const said = kind[what.on];
  return said === undefined ? name : `${said} ${name}`;
}

/** The name of the thing a menu was opened on: what it carries, or what its rows are named by. */
function nameOf(what: MenuOn, rows: readonly Offer[]): string | undefined {
  switch (what.on) {
    case "persona":
      return what.persona;
    case "vault":
      return what.vault;
    case "clone":
    case "absent":
      return what.repo;
    case "workspace":
      return what.workspace;
    case "file":
      return what.at.path.split("/").pop() || undefined;
    case "root":
    case "pane":
    case "refusal":
    case "finished":
    case "view":
      return undefined;
    default:
      return rows.find((row) => row.name !== undefined && !NAMES_ANOTHER.has(stemOf(row.id)))?.name;
  }
}

/** Whether a row is drawn at all: one that could only say what a press would do is not. */
function drawnAtAll(offer: Offer): boolean {
  return offer.available || DRESS[stemOf(offer.id)]?.quiet !== true;
}

/**
 * Whether a menu on that thing would draw nothing, so the thing is drawn with no menu of its
 * own. Cheap, and asked per tab per render: the look-ups of {@link menuRows} and no scan.
 */
export function menuIsEmpty(what: MenuOn, offers: Catalogued): boolean {
  if (curateSubjectOf(what) !== undefined) return false;
  const { above, below } = menuRows(what, offers);
  return ![...above, ...below].some(drawnAtAll);
}

/**
 * **A menu as it is drawn**: the catalogue's rows for the thing it was opened on, each with its
 * words, in at most four groups, with the rows of one kind folded into a submenu.
 *
 * **Made only for a menu that is open.** The Ask, Curate and Move to rows are scans of the
 * catalogue, which a strip of fifty tabs must not pay per render: `Menus.tsx` calls this from
 * inside the menu's content, which Radix mounts only while the menu is up.
 */
export function menuOf(what: MenuOn, offers: Catalogued): MenuDrawn {
  const { above, below } = menuRows(what, offers);
  const slots: Record<Slot, EntryDrawn[]> = { first: [], open: [], run: [], end: [] };
  const subs = new Map<SubKey, SubDrawn>();
  const folded = (key: SubKey, slot: Slot): SubDrawn => {
    const had = subs.get(key);
    if (had !== undefined) return had;
    const sub: SubDrawn = {
      kind: "sub",
      key,
      said: SUBMENU[key].said,
      tip: SUBMENU[key].tip,
      groups: [{ entries: [] }],
    };
    subs.set(key, sub);
    slots[slot].push(sub);
    return sub;
  };
  const place = (offer: Offer, isBelow: boolean) => {
    if (!drawnAtAll(offer)) return;
    const dress = DRESS[stemOf(offer.id)];
    const row = drawnRow(offer, dress, isBelow);
    const slot =
      dress === undefined
        ? isBelow
          ? "end"
          : "first"
        : typeof dress.slot === "function"
          ? dress.slot(offer)
          : dress.slot;
    if (dress?.under === undefined) slots[slot].push(row);
    else folded(dress.under, slot).groups[0].entries.push(row);
  };
  above.forEach((offer) => place(offer, false));
  below.forEach((offer) => place(offer, true));

  // **Ask ▸**, on a chat's tab: one row per persona this chat may ask, named by the persona
  // alone. What asking does is the same for each, so the submenu's own tooltip says it once.
  if (what.on === "chat") {
    const asks = askRows(what.tab, offers);
    if (asks.length > 0)
      folded("ask", "open").groups[0].entries.push(
        ...asks.map((offer) =>
          plain(offer, offer.does.verb === "askPersona" ? offer.does.persona : offer.title, true),
        ),
      );
  }
  const curating = curateSubjectOf(what);
  const curate = curating === undefined ? undefined : curateSub(curating, offers);
  if (curate !== undefined) slots.open.push(curate);
  // **Move to ▸**, on a memory's row (#1190): a row per store but its own, named as the
  // memory's tab names the store.
  if (what.on === "memory") {
    const moves = moveRows(what.key, offers);
    if (moves.length > 0)
      folded("moveTo", "open").groups[0].entries.push(
        ...moves.map((offer) =>
          plain(offer, offer.does.verb === "moveMemory" ? storeLabel(offer.does.to) : offer.title),
        ),
      );
  }

  // **The last group ends with the row that ends the thing itself**: a submenu of the ways to
  // stop it stands above End, not under it.
  slots.end.sort((one, other) => Number(other.kind === "sub") - Number(one.kind === "sub"));

  const name = nameOf(what, [...above, ...below]);
  return {
    about: aboutOf(what, name),
    name,
    groups: SLOTS.map((slot) => ({ entries: slots[slot] })).filter(
      (group) => group.entries.length > 0,
    ),
  };
}

/** Every row of a menu, its submenus' too, in the order they are drawn. */
export function rowsOf(menu: MenuDrawn | SubDrawn): RowDrawn[] {
  return menu.groups.flatMap((group) =>
    group.entries.flatMap((entry) => (entry.kind === "row" ? [entry] : rowsOf(entry))),
  );
}
