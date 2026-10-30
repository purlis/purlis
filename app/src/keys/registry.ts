// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The key registry** (#2921, spec #2897 N24, N25): every shortcut the window answers,
 * declared once, with where it applies and what it yields to.
 *
 * - **One entry per act**: its id, what it does in the words the cheat sheet says, the group the
 *   sheet lists it under, its chords on a Mac and everywhere else (Ctrl takes ⌘'s place off a
 *   Mac, except where a chat would lose the chord), how it is spelled, the catalogue rows that
 *   show it, and how it is recognised.
 * - **Where it applies** (`where`): the whole window, or one place in it (the palette, ⌘P's
 *   files, a dialog, a menu, a focused tab, a list's row, a chat's terminal, an editor, a form).
 * - **What it yields to** (`yields`): a modal dialog that is up (the operator's ruling of
 *   2026-10-24), any dialog that has the keyboard, the palette while it is up, a chat's terminal
 *   (always, for its Ctrl chords, or off a Mac only), a tab that renames on the key, an editor, a
 *   field it would be typed into. A key that yields does nothing there: the event goes on to whoever
 *   has the keyboard, as any key the window has no use for does.
 * - **How it is recognised** (`matches`): by the function that has always recognised it, beside
 *   its reasons (`sideKeys.ts`, `taskKeys.ts`, `workspaceSwitch.ts` and the rest), so a layout's
 *   edge cases stay where they are argued. A **path** is a sequence of keys already declared
 *   (⌘P then ↩), listed for the sheet; a **native** key is the system menu's or an editor's own,
 *   declared so a new key cannot be put on it.
 *
 * Each handler asks the registry ({@link pressed}) which of its acts a keystroke is, with the
 * context rules applied here, so a rule is one rule wherever its handler is. The cheat sheet
 * (`ShortcutsView.tsx`), the palette's rows and every menu's rows (`MenuShortcuts`) read the keys
 * from here. `registry.test.ts` fails on a conflict: two acts on one chord in one place, neither
 * yielding to the other.
 */
import { onAMac } from "../tabKeys";
import { pinOfKey, opensTheWorkspaces, workspacesKeySaid } from "../workspaceSwitch";
import { opensTheSwitcher, switcherKeySaid } from "../switcherKey";
import { sideKeyOf, sideKeysSaid, type SideKey } from "../sideKeys";
import { TASK_KEY_ROW, taskKeyOf, taskKeySaid, type TaskKey } from "../taskKeys";
import { f6Of } from "../regionCycle";
import { opensAShell, shellKeySaid } from "../shellKey";
import { sizeKey, type SizeKey } from "../textSize";
import { isRevertKey, revertKeySaid } from "../themeDrafts";
import { submits, submitKeySaid } from "../ui/field/submitKey";
import { asksForAMenu } from "../ui/menu/ContextMenu";
import { asksForTheMenu } from "../terminalRows";
import { fileKeyOf, fileKeySaid, opensIt } from "./paletteKeys";
import { opensFind, opensFindInAFile } from "./findKeys";
import { viewHistoryKeyOf } from "./viewKeys";
import { focusKeySaid, opensTheShortcuts, shortcutsKeySaid, togglesFocus } from "./ownKeys";
import {
  aModalIsUp,
  inADialog,
  inAChat,
  inAMenu,
  inAnEditor,
  onARenamingTab,
  typedInto,
} from "./contexts";

/** The two keyboards a key is declared for: a Mac's, and everyone else's. */
export type Platform = "mac" | "other";

/** The places a key applies in. `window` is everywhere; each other is inside it. */
export const SCOPES = [
  "window",
  "palette",
  "finder",
  "dialog",
  "menu",
  "tab",
  "row",
  "terminal",
  "editor",
  "form",
  "view",
] as const;
export type Scope = (typeof SCOPES)[number];

/** What a key stands back for. */
export type Yield =
  /** A modal dialog is up (the dialog family's root, an alert dialog). */
  | "modal"
  /** Any dialog has the keyboard, a popover's included. */
  | "dialog"
  /** The palette is up: its keys are its own. */
  | "palette"
  /** The keystroke landed in a chat's terminal. */
  | "terminal"
  /** …and is a Ctrl chord (`Palette.theChatKeepsIt`, charter-app#106). */
  | "terminal-ctrl"
  /** …off a Mac, where this chord is a Ctrl chord a terminal has a use for. */
  | "terminal-off-mac"
  /** The keystroke landed on a tab that answers the key itself (`F2` renames it). */
  | "tab"
  /** The keystroke landed in a text editor, whose keymap has it. */
  | "editor"
  /** …and is one of the act's `editorKeeps` chords, which the editor's keymap binds. */
  | "editor-chords"
  /** The keystroke landed in a field or an editor, and the act cannot run now. */
  | "typing"
  /** The keystroke landed in an open menu. */
  | "menu";

/** One chord: the key as it types (or by its place, `code`) and the modifiers held. */
export interface Chord {
  key: string;
  /** The physical key, for a chord read by its place (⌥ turns a Mac's letter into another). */
  code?: string;
  meta?: true;
  ctrl?: true;
  alt?: true;
  shift?: true;
}

/** The cheat sheet's groups, in its order. */
export const GROUPS = [
  "Go to",
  "Window",
  "Tabs and chats",
  "Palette",
  "Dialogs and forms",
] as const;
export type Group = (typeof GROUPS)[number];

/** What a handler knows that the page does not say. */
export interface Held {
  /** The keyboard's platform; the page's, when not said. */
  mac?: boolean;
  /** The palette is up. */
  palette?: boolean;
  /** The modal that is up is this handler's own (the palette's keys while it is up). */
  ownModal?: boolean;
  /** The act can run now: a key that cannot is left to a field it lands in (`typing`). */
  runnable?: boolean;
}

/** **A shortcut**, as the registry declares it. */
export interface Shortcut {
  id: string;
  /** What it does, as the cheat sheet says it. */
  does: string;
  group: Group;
  where: Scope;
  /** `key` is matched by its handler; a `path` is keys already declared, in turn; a `native`
   *  key is the system menu's or an editor's own. */
  kind: "key" | "path" | "native";
  chords: Readonly<Record<Platform, readonly Chord[]>>;
  /** How a row, a tooltip and the sheet spell it. */
  said: (mac: boolean) => string;
  /** Its other chords, as the sheet spells them beside the first. */
  alsoSaid?: (mac: boolean) => readonly string[];
  yields: readonly Yield[];
  /** The chords an editor keeps from this act (`editor-chords`): its keymap's, on each keyboard. */
  editorKeeps?: Readonly<Record<Platform, readonly Chord[]>>;
  /** The catalogue rows whose palette and menu rows show it: an id, or a prefix ending in `:`. */
  rows?: readonly string[];
  matches?: (e: KeyboardEvent, mac: boolean) => boolean;
}

// --- Chords ----------------------------------------------------------------------------------

/** ⌘ and a key, with `more` held too. */
const cmd = (key: string, more: Omit<Chord, "key" | "meta"> = {}): Chord => ({
  key,
  meta: true,
  ...more,
});
/** Ctrl and a key, with `more` held too. */
const ctl = (key: string, more: Omit<Chord, "key" | "ctrl"> = {}): Chord => ({
  key,
  ctrl: true,
  ...more,
});
/** A key alone, or with `more` held. */
const bare = (key: string, more: Omit<Chord, "key"> = {}): Chord => ({ key, ...more });
/** The same chords on both keyboards. */
const both = (...chords: Chord[]) => ({ mac: chords, other: chords });
/** The keys every list, tree and menu walks its rows with (React Aria's). */
const NAVIGATION = ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End"];
/** The same spelling on both keyboards. */
const says = (word: string) => () => word;

const isSide = (want: SideKey) => (e: KeyboardEvent, mac: boolean) => {
  const got = sideKeyOf(e, mac);
  if (got === undefined) return false;
  if ("toggle" in want) return "toggle" in got && got.toggle === want.toggle;
  return "show" in got && got.show === want.show;
};
const isTask = (want: TaskKey) => (e: KeyboardEvent, mac: boolean) => taskKeyOf(e, mac) === want;
const isSize = (want: SizeKey) => (e: KeyboardEvent, mac: boolean) => sizeKey(e, mac) === want;
const sideSaid = (which: keyof ReturnType<typeof sideKeysSaid>) => (mac: boolean) =>
  sideKeysSaid(mac)[which] ?? "";
/** Use skill…'s chords (#3954): ⌘⇧S, and Ctrl+Shift+S off a Mac. */
const USE_SKILL = { mac: cmd("S", { shift: true }), other: ctl("S", { shift: true }) };
/** No modifier held. */
const plain = (e: KeyboardEvent) => !e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey;

// --- The declarations ------------------------------------------------------------------------

/** **Every shortcut**, in the cheat sheet's order within each group. */
export const SHORTCUTS: readonly Shortcut[] = [
  // Go to
  {
    id: "palette.open",
    does: "Open the command palette",
    group: "Go to",
    where: "window",
    kind: "key",
    chords: { mac: [cmd("k"), ctl("k"), bare("F2")], other: [ctl("k"), bare("F2")] },
    said: (mac) => (mac ? "⌘K" : "Ctrl+K"),
    alsoSaid: () => ["F2"],
    // A modal's keys are its own; the palette's own second press is the palette's (`handBack`);
    // Ctrl+K is a chat's kill-line; a chat's tab renames on F2; and in a code editor the editor
    // keeps its deleteLine (⌘⇧K, Ctrl+Shift+K) and a Mac's deleteToLineEnd (Ctrl+K), as VS Code
    // leaves them to its editor (D-2921-25). ⌘K is the palette's there too.
    yields: ["modal", "palette", "terminal-ctrl", "tab", "editor-chords"],
    editorKeeps: {
      mac: [cmd("k", { shift: true }), ctl("k")],
      other: [ctl("k", { shift: true })],
    },
    matches: (e) => opensIt(e),
  },
  {
    id: "finder.open",
    does: "Find a project or a file; again, the next one down",
    group: "Go to",
    where: "window",
    kind: "key",
    chords: { mac: [cmd("p")], other: [ctl("P", { shift: true })] },
    said: switcherKeySaid,
    yields: ["modal"],
    rows: ["project.switch"],
    matches: opensTheSwitcher,
  },
  {
    id: "finder.back",
    does: "Back to the last project",
    group: "Go to",
    where: "window",
    kind: "path",
    chords: { mac: [], other: [] },
    said: (mac) => `${switcherKeySaid(mac)} ${mac ? "↩" : "Enter"}`,
    yields: [],
  },
  {
    id: "workspaces.open",
    does: "Open the project's workspaces in the title bar",
    group: "Go to",
    where: "window",
    kind: "key",
    chords: {
      mac: [cmd("p", { alt: true, code: "KeyP" })],
      other: [ctl("P", { alt: true, shift: true, code: "KeyP" })],
    },
    said: workspacesKeySaid,
    // Taken from a chat's terminal on purpose, as VS Code takes its own app keys (the operator's
    // decision of 2026-10-24, `workspaceSwitch.opensTheWorkspaces`).
    yields: ["modal", "palette"],
    matches: opensTheWorkspaces,
  },
  {
    id: "workspace.pin",
    does: "Go to the first nine pinned workspaces",
    group: "Go to",
    where: "window",
    kind: "key",
    chords: {
      mac: ["1", "2", "3", "4", "5", "6", "7", "8", "9"].map((n) => cmd(n)),
      other: ["1", "2", "3", "4", "5", "6", "7", "8", "9"].map((n) => ctl(n)),
    },
    said: (mac) => (mac ? "⌘1–⌘9" : "Ctrl+1–Ctrl+9"),
    yields: ["modal", "palette", "terminal-off-mac"],
    matches: (e, mac) => pinOfKey(e, mac) !== undefined,
  },
  {
    id: "region.next",
    does: "Focus the next region",
    group: "Go to",
    where: "window",
    kind: "key",
    chords: both(bare("F6")),
    said: sideSaid("next"),
    yields: ["dialog", "terminal"],
    rows: ["view.region:next"],
    matches: (e) => f6Of(e)?.back === false,
  },
  {
    id: "region.previous",
    does: "Focus the previous region",
    group: "Go to",
    where: "window",
    kind: "key",
    chords: both(bare("F6", { shift: true })),
    said: sideSaid("previous"),
    yields: ["dialog", "terminal"],
    rows: ["view.region:previous"],
    matches: (e) => f6Of(e)?.back === true,
  },
  {
    id: "view.back",
    does: "Back to the last page in this view tab",
    group: "Go to",
    where: "view",
    kind: "key",
    chords: { mac: [cmd("[")], other: [bare("ArrowLeft", { alt: true })] },
    said: (mac) => (mac ? "⌘[" : "Alt+←"),
    // An editor keeps its own ⌘[ (indentLess) and Alt+← (cursorSyntaxLeft); a dialog or a menu
    // opened from the view keeps the keys it is handed.
    yields: ["modal", "dialog", "menu", "editor"],
    matches: (e, mac) => viewHistoryKeyOf(e, mac) === "back",
  },
  {
    id: "view.forward",
    does: "Forward again, in this view tab",
    group: "Go to",
    where: "view",
    kind: "key",
    chords: { mac: [cmd("]")], other: [bare("ArrowRight", { alt: true })] },
    said: (mac) => (mac ? "⌘]" : "Alt+→"),
    yields: ["modal", "dialog", "menu", "editor"],
    matches: (e, mac) => viewHistoryKeyOf(e, mac) === "forward",
  },
  // Window
  {
    id: "view.explorer",
    does: "Show the Explorer view",
    group: "Window",
    where: "window",
    kind: "key",
    chords: { mac: [cmd("E", { shift: true })], other: [ctl("E", { shift: true })] },
    said: sideSaid("explorer"),
    yields: ["dialog"],
    rows: ["view.show:explorer"],
    matches: isSide({ show: "explorer" }),
  },
  {
    id: "view.chats",
    does: "Show the Chats view",
    group: "Window",
    where: "window",
    kind: "key",
    chords: { mac: [cmd("C", { shift: true })], other: [ctl("C", { shift: true })] },
    said: sideSaid("chats"),
    // Ctrl+Shift+C is a Linux terminal's copy.
    yields: ["dialog", "terminal-off-mac"],
    rows: ["view.show:chats"],
    matches: isSide({ show: "chats" }),
  },
  {
    id: "view.search",
    does: "Search in files",
    group: "Window",
    where: "window",
    kind: "key",
    chords: { mac: [cmd("F", { shift: true })], other: [ctl("F", { shift: true })] },
    said: sideSaid("search"),
    // Off a Mac, inside a chat, Ctrl+Shift+F is the chat's find bar (`searchKey.ts`).
    yields: ["dialog", "terminal-off-mac"],
    rows: ["search.files"],
    matches: isSide({ show: "search" }),
  },
  {
    id: "view.changes",
    does: "Show the Changes view",
    group: "Window",
    where: "window",
    kind: "key",
    chords: both(ctl("G", { shift: true })),
    said: sideSaid("changes"),
    yields: ["dialog"],
    rows: ["view.show:changes"],
    matches: isSide({ show: "changes" }),
  },
  {
    id: "view.inbox",
    does: "Show the Inbox",
    group: "Window",
    where: "window",
    kind: "key",
    chords: { mac: [cmd("I", { shift: true })], other: [ctl("I", { shift: true })] },
    said: sideSaid("inbox"),
    yields: ["dialog"],
    rows: ["view.show:inbox"],
    matches: isSide({ show: "inbox" }),
  },
  {
    id: "region.navigation",
    does: "Put the left sidebar away, or bring it back",
    group: "Window",
    where: "window",
    kind: "key",
    chords: { mac: [cmd("b")], other: [ctl("b")] },
    said: sideSaid("navigation"),
    // Ctrl+B is readline's back-char and tmux's prefix.
    yields: ["dialog", "terminal-off-mac"],
    rows: ["view.toggle:navigation"],
    matches: isSide({ toggle: "navigation" }),
  },
  {
    id: "region.aside",
    does: "Put the right sidebar away, or bring it back",
    group: "Window",
    where: "window",
    kind: "key",
    chords: {
      mac: [cmd("b", { alt: true, code: "KeyB" })],
      other: [ctl("b", { alt: true, code: "KeyB" })],
    },
    said: sideSaid("aside"),
    yields: ["dialog", "terminal-off-mac"],
    rows: ["view.toggle:aside"],
    matches: isSide({ toggle: "aside" }),
  },
  {
    id: "region.panel",
    does: "Show or hide the Panel",
    group: "Window",
    where: "window",
    kind: "key",
    chords: { mac: [cmd("j")], other: [ctl("j")] },
    said: sideSaid("panel"),
    // Ctrl+J is a terminal's newline.
    yields: ["dialog", "terminal-off-mac"],
    rows: ["view.toggle:panel"],
    matches: isSide({ toggle: "panel" }),
  },
  {
    id: "focus.toggle",
    does: "Focus mode: the work alone, and back",
    group: "Window",
    where: "window",
    kind: "key",
    chords: {
      mac: [cmd("Enter", { shift: true })],
      other: [ctl("Enter", { shift: true })],
    },
    said: focusKeySaid,
    yields: ["modal"],
    rows: ["focus.toggle"],
    matches: togglesFocus,
  },
  {
    id: "shortcuts.show",
    does: "Show the keyboard shortcuts",
    group: "Window",
    where: "window",
    kind: "key",
    chords: { mac: [cmd("/")], other: [ctl("/")] },
    said: shortcutsKeySaid,
    // An editor comments a line on it (CodeMirror's `Mod-/`).
    yields: ["modal", "editor"],
    rows: ["shortcuts.show"],
    matches: opensTheShortcuts,
  },
  {
    id: "text.bigger",
    does: "Make the text bigger (a terminal's, in a terminal)",
    group: "Window",
    where: "window",
    kind: "key",
    chords: {
      mac: [cmd("="), cmd("+", { shift: true })],
      other: [ctl("="), ctl("+", { shift: true })],
    },
    said: (mac) => (mac ? "⌘=" : "Ctrl+="),
    // Never held back: a person who cannot read a dialog needs it bigger (D-2921-4).
    yields: [],
    matches: isSize("bigger"),
  },
  {
    id: "text.smaller",
    does: "Make the text smaller",
    group: "Window",
    where: "window",
    kind: "key",
    chords: { mac: [cmd("-")], other: [ctl("-")] },
    said: (mac) => (mac ? "⌘-" : "Ctrl+-"),
    yields: [],
    matches: isSize("smaller"),
  },
  {
    id: "text.reset",
    does: "Text at its usual size",
    group: "Window",
    where: "window",
    kind: "key",
    chords: { mac: [cmd("0")], other: [ctl("0")] },
    said: (mac) => (mac ? "⌘0" : "Ctrl+0"),
    yields: [],
    matches: isSize("reset"),
  },
  {
    id: "theme.revert",
    does: "Drop the theme drafts here at once",
    group: "Window",
    where: "window",
    kind: "key",
    chords: {
      mac: [cmd("Backspace", { shift: true })],
      other: [ctl("Backspace", { shift: true })],
    },
    said: revertKeySaid,
    // Reserved (spec #2313): it works even when a draft has made the window hard to read.
    yields: [],
    matches: isRevertKey,
  },
  {
    id: "settings.open",
    does: "Open Settings",
    group: "Window",
    where: "window",
    kind: "native",
    chords: { mac: [cmd(",")], other: [ctl(",")] },
    said: (mac) => (mac ? "⌘," : "Ctrl+,"),
    yields: [],
    rows: ["settings.show"],
  },
  {
    id: "app.hide",
    does: "Hide purlis",
    group: "Window",
    where: "window",
    kind: "native",
    chords: { mac: [cmd("h")], other: [] },
    said: (mac) => (mac ? "⌘H" : ""),
    yields: [],
  },
  {
    id: "app.quit",
    does: "Quit purlis",
    group: "Window",
    where: "window",
    kind: "native",
    chords: { mac: [cmd("q")], other: [ctl("Q", { shift: true })] },
    said: (mac) => (mac ? "⌘Q" : "Ctrl+Shift+Q"),
    yields: [],
  },
  // Tabs and chats
  {
    id: "shell.new",
    does: "Open a shell tab",
    group: "Tabs and chats",
    where: "window",
    kind: "key",
    chords: { mac: [cmd("T", { shift: true })], other: [ctl("T", { shift: true })] },
    said: shellKeySaid,
    yields: ["modal"],
    rows: ["shell.new"],
    matches: opensAShell,
  },
  {
    id: "chat.useSkill",
    does: "Use one of the chat's skills: Use skill…",
    group: "Tabs and chats",
    where: "window",
    kind: "key",
    // `S` for skill (#3954). ⌘⇧S was no key of the window's; Ctrl with Shift and a letter is no
    // byte to a chat's terminal, so it is the window's while a chat has the keyboard, as
    // Ctrl+Shift+T is. An editor keeps it, for a Save As of its own.
    chords: { mac: [USE_SKILL.mac], other: [USE_SKILL.other] },
    said: (mac) => (mac ? "⌘⇧S" : "Ctrl+Shift+S"),
    yields: ["modal", "editor"],
    // Each chat tab's row (`tab.useskill:<tab>`): the key presses the one in front.
    rows: ["tab.useskill:"],
    matches: (e, mac) => isChord(e, mac ? USE_SKILL.mac : USE_SKILL.other),
  },
  {
    id: "tasks.menu",
    does: "The menu of this tab's chats",
    group: "Tabs and chats",
    where: "window",
    kind: "key",
    chords: { mac: [cmd("J", { shift: true })], other: [ctl("J", { shift: true })] },
    said: (mac) => taskKeySaid("menu", mac),
    yields: ["dialog", "typing"],
    rows: [TASK_KEY_ROW.menu],
    matches: isTask("menu"),
  },
  {
    id: "tasks.next",
    does: "The next chat in this tab",
    group: "Tabs and chats",
    where: "window",
    kind: "key",
    chords: {
      mac: [cmd("ArrowDown", { alt: true })],
      other: [ctl("]", { shift: true, code: "BracketRight" })],
    },
    said: (mac) => taskKeySaid("next", mac),
    yields: ["dialog", "typing"],
    rows: [TASK_KEY_ROW.next],
    matches: isTask("next"),
  },
  {
    id: "tasks.previous",
    does: "The previous chat in this tab",
    group: "Tabs and chats",
    where: "window",
    kind: "key",
    chords: {
      mac: [cmd("ArrowUp", { alt: true })],
      other: [ctl("[", { shift: true, code: "BracketLeft" })],
    },
    said: (mac) => taskKeySaid("previous", mac),
    yields: ["dialog", "typing"],
    rows: [TASK_KEY_ROW.previous],
    matches: isTask("previous"),
  },
  {
    id: "tasks.own",
    does: "Back to the session's own chat",
    group: "Tabs and chats",
    where: "window",
    kind: "key",
    chords: { mac: [cmd("H", { shift: true })], other: [ctl("H", { shift: true })] },
    said: (mac) => taskKeySaid("own", mac),
    yields: ["dialog", "typing"],
    rows: [TASK_KEY_ROW.own],
    matches: isTask("own"),
  },
  {
    id: "tab.rename",
    does: "Rename the focused chat tab",
    group: "Tabs and chats",
    where: "tab",
    kind: "key",
    chords: both(bare("F2")),
    said: says("F2"),
    yields: [],
    rows: ["tab.rename:"],
    matches: (e) => e.key === "F2" && plain(e),
  },
  {
    id: "tab.close",
    does: "Close the focused tab",
    group: "Tabs and chats",
    where: "tab",
    kind: "key",
    chords: { mac: [bare("Backspace"), bare("Delete")], other: [bare("Delete")] },
    said: (mac) => (mac ? "⌫" : "Delete"),
    yields: [],
    rows: ["tab.close:", "project.close:"],
    matches: (e, mac) => plain(e) && (e.key === "Delete" || (e.key === "Backspace" && mac)),
  },
  {
    id: "chat.find",
    does: "Find in the chat",
    group: "Tabs and chats",
    where: "terminal",
    kind: "key",
    chords: { mac: [cmd("f")], other: [ctl("F", { shift: true })] },
    said: (mac) => (mac ? "⌘F" : "Ctrl+Shift+F"),
    yields: [],
    matches: opensFind,
  },
  {
    id: "chat.menu",
    does: "The chat's menu, under its cursor",
    group: "Tabs and chats",
    where: "terminal",
    kind: "key",
    chords: both(bare("F10", { shift: true }), bare("ContextMenu")),
    said: says("Shift+F10"),
    yields: [],
    // Whether a program has the mouse is the pane's to say; asked here as if none had it.
    matches: (e) => asksForTheMenu(e, false),
  },
  {
    id: "chat.newline",
    does: "A new line in the chat's prompt",
    group: "Tabs and chats",
    where: "terminal",
    kind: "key",
    chords: both(bare("Enter", { shift: true })),
    said: (mac) => (mac ? "⇧↩" : "Shift+Enter"),
    yields: [],
    matches: (e) => e.key === "Enter" && e.shiftKey && !e.ctrlKey && !e.altKey && !e.metaKey,
  },
  {
    id: "chat.leave",
    does: "Out of the chat's terminal, a browser you drive or DevTools (with Shift, back)",
    group: "Tabs and chats",
    where: "terminal",
    kind: "key",
    chords: both(ctl("Tab"), ctl("Tab", { shift: true })),
    said: says("Ctrl+Tab"),
    alsoSaid: () => ["Ctrl+Shift+Tab"],
    yields: [],
    matches: (e) => e.key === "Tab" && e.ctrlKey && !e.metaKey && !e.altKey,
  },
  {
    id: "file.find",
    does: "Find in the file",
    group: "Tabs and chats",
    where: "editor",
    kind: "key",
    chords: { mac: [cmd("f")], other: [ctl("f")] },
    said: (mac) => (mac ? "⌘F" : "Ctrl+F"),
    yields: [],
    matches: opensFindInAFile,
  },
  {
    id: "editor.comment",
    does: "Comment the line out, in an editor",
    group: "Tabs and chats",
    where: "editor",
    kind: "native",
    chords: { mac: [cmd("/")], other: [ctl("/")] },
    said: shortcutsKeySaid,
    yields: [],
  },
  {
    id: "row.menu",
    does: "The focused row's menu",
    group: "Tabs and chats",
    where: "row",
    kind: "key",
    chords: both(bare("F10", { shift: true }), bare("ContextMenu")),
    said: says("Shift+F10"),
    yields: [],
    matches: (e) => asksForAMenu(e),
  },
  {
    id: "row.move",
    does: "Walk a list's rows, to its first and its last",
    group: "Tabs and chats",
    where: "row",
    kind: "native",
    chords: both(...NAVIGATION.map((key) => bare(key))),
    said: says("↑ ↓ ← →"),
    alsoSaid: () => ["Home", "End"],
    yields: [],
  },
  {
    id: "tab.move",
    does: "Move along the tabs, to the first and the last",
    group: "Tabs and chats",
    where: "tab",
    kind: "native",
    chords: both(...["ArrowLeft", "ArrowRight", "Home", "End"].map((key) => bare(key))),
    said: says("← →"),
    alsoSaid: () => ["Home", "End"],
    yields: [],
  },
  // Palette
  {
    id: "palette.run",
    does: "Run the row aimed at",
    group: "Palette",
    where: "palette",
    kind: "key",
    chords: both(bare("Enter")),
    said: (mac) => (mac ? "↩" : "Enter"),
    yields: [],
    matches: (e) => e.key === "Enter",
  },
  {
    id: "palette.aim",
    does: "Aim at the row above or below",
    group: "Palette",
    where: "palette",
    kind: "path",
    chords: { mac: [], other: [] },
    said: says("↑ ↓"),
    yields: [],
  },
  {
    id: "palette.close",
    does: "Close the palette, the keyboard back where it was",
    group: "Palette",
    where: "palette",
    kind: "key",
    chords: both(bare("Escape")),
    said: says("Esc"),
    yields: [],
    matches: (e) => e.key === "Escape",
  },
  {
    id: "palette.handBack",
    does: "Send F2 to the chat in front",
    group: "Palette",
    where: "palette",
    kind: "key",
    chords: both(bare("F2")),
    said: says("F2"),
    yields: [],
    rows: ["pane.sendkey"],
    matches: (e) => e.key === "F2" && plain(e),
  },
  {
    id: "finder.copyPath",
    does: "Copy the aimed file's path",
    group: "Palette",
    where: "finder",
    kind: "key",
    chords: {
      mac: [cmd("c", { alt: true, code: "KeyC" })],
      other: [bare("c", { alt: true, shift: true, code: "KeyC" })],
    },
    said: (mac) => fileKeySaid("copy", mac),
    yields: [],
    matches: (e, mac) => fileKeyOf(e, mac) === "copy",
  },
  {
    id: "finder.reveal",
    does: "Show the aimed file in the file manager",
    group: "Palette",
    where: "finder",
    kind: "key",
    chords: {
      mac: [cmd("r", { alt: true, code: "KeyR" })],
      other: [bare("r", { alt: true, shift: true, code: "KeyR" })],
    },
    said: (mac) => fileKeySaid("reveal", mac),
    yields: [],
    matches: (e, mac) => fileKeyOf(e, mac) === "reveal",
  },
  {
    id: "finder.widen",
    does: "Find files in a wider place (with Shift, narrower)",
    group: "Palette",
    where: "finder",
    kind: "path",
    chords: { mac: [], other: [] },
    said: says("Tab"),
    alsoSaid: () => ["Shift+Tab"],
    yields: [],
  },
  // Dialogs and forms
  {
    id: "form.submit",
    does: "Send the form: its main action",
    group: "Dialogs and forms",
    where: "form",
    kind: "key",
    chords: { mac: [cmd("Enter")], other: [ctl("Enter")] },
    said: submitKeySaid,
    yields: [],
    matches: (e, mac) => submits(e, mac),
  },
  {
    id: "dialog.default",
    does: "Press the dialog's default button",
    group: "Dialogs and forms",
    where: "dialog",
    kind: "key",
    chords: both(bare("Enter")),
    said: (mac) => (mac ? "↩" : "Enter"),
    // In an editor in a dialog, Enter is a new line (CodeMirror's insertNewline).
    yields: ["palette", "editor"],
    matches: (e) => e.key === "Enter" && plain(e),
  },
  {
    id: "dialog.close",
    does: "Close the dialog, or the menu or popover in it",
    group: "Dialogs and forms",
    where: "dialog",
    kind: "key",
    chords: both(bare("Escape")),
    said: says("Esc"),
    yields: ["menu", "palette"],
    matches: (e) => e.key === "Escape",
  },
  {
    id: "menu.move",
    does: "Walk a menu's rows; → opens a submenu, ← closes it",
    group: "Dialogs and forms",
    where: "menu",
    kind: "native",
    chords: both(...NAVIGATION.map((key) => bare(key))),
    said: says("↑ ↓ ← →"),
    alsoSaid: () => ["Home", "End"],
    yields: [],
  },
  {
    id: "menu.close",
    does: "Close the menu, the keyboard back on what opened it",
    group: "Dialogs and forms",
    where: "menu",
    kind: "key",
    chords: both(bare("Escape")),
    said: says("Esc"),
    yields: [],
    matches: (e) => e.key === "Escape",
  },
];

/** The side keys (`sideKeys.ts`), each its act: one handler carries them all out. */
export const SIDE_KEY_IDS: readonly string[] = [
  "view.search",
  "view.changes",
  "region.navigation",
  "region.aside",
  "region.panel",
  "view.explorer",
  "view.chats",
  "view.inbox",
];

// --- Reading it ------------------------------------------------------------------------------

const BY_ID = new Map(SHORTCUTS.map((one) => [one.id, one]));

/** The registry's entry `id`. Throws on an id it does not hold: a typo is a bug, not a key. */
export function shortcut(id: string): Shortcut {
  const one = BY_ID.get(id);
  if (one === undefined) throw new Error(`no shortcut "${id}" in the key registry`);
  return one;
}

/** How `id`'s key is spelled on this keyboard. */
export function keySaid(id: string, mac: boolean = onAMac()): string {
  return shortcut(id).said(mac);
}

/**
 * **The key a catalogue row shows** (N25), by the row's id: the palette's rows and every menu's
 * (`MenuShortcuts`). A row has a key where an entry names its id, or a prefix of it ending in
 * `:` (each tab's `tab.close:<id>`). A key with no chord on this keyboard says nothing.
 */
export function rowKey(row: string, mac: boolean = onAMac(), anywhere = false): string | undefined {
  for (const one of SHORTCUTS) {
    if (anywhere && one.where !== "window") continue;
    const named = one.rows?.some((id) => (id.endsWith(":") ? row.startsWith(id) : row === id));
    if (!named || (one.chords[mac ? "mac" : "other"].length === 0 && one.kind !== "path")) continue;
    const said = one.said(mac);
    if (said !== "") return said;
  }
  return undefined;
}

/**
 * A chord as `aria-keyshortcuts` names it (WAI-ARIA 1.2): the modifiers by their UI Events names,
 * then the key by its value, as `Shift+Meta+T` or `Control+/`.
 */
export function ariaChord(chord: Chord): string {
  const key =
    chord.key === " " ? "Space" : chord.key.length === 1 ? chord.key.toUpperCase() : chord.key;
  return [
    chord.ctrl && "Control",
    chord.alt && "Alt",
    chord.shift && "Shift",
    chord.meta && "Meta",
    key,
  ]
    .filter(Boolean)
    .join("+");
}

/** **The key a catalogue row shows, as a reader is told it** (`aria-keyshortcuts`). */
export function rowAriaKeys(
  row: string,
  mac: boolean = onAMac(),
  anywhere = false,
): string | undefined {
  const platform = mac ? "mac" : "other";
  const one = SHORTCUTS.find(
    (entry) =>
      (!anywhere || entry.where === "window") &&
      entry.rows?.some((id) => (id.endsWith(":") ? row.startsWith(id) : row === id)),
  );
  const chords = one?.chords[platform] ?? [];
  return chords.length === 0 ? undefined : chords.map(ariaChord).join(" ");
}

/** How each symbol of a key's spelling is said, on a Mac and elsewhere. */
const SYMBOL_WORDS: Readonly<Record<string, readonly [string, string]>> = {
  "⌘": ["Command", "Command"],
  "⇧": ["Shift", "Shift"],
  "⌥": ["Option", "Alt"],
  "⌃": ["Control", "Control"],
  "↩": ["Return", "Enter"],
  "⌫": ["Delete", "Backspace"],
  "↑": ["Up Arrow", "Up Arrow"],
  "↓": ["Down Arrow", "Down Arrow"],
  "←": ["Left Arrow", "Left Arrow"],
  "→": ["Right Arrow", "Right Arrow"],
  "/": ["Slash", "Slash"],
  "=": ["Equals", "Equals"],
  "-": ["Minus", "Minus"],
  ",": ["Comma", "Comma"],
  "[": ["Left Bracket", "Left Bracket"],
  "]": ["Right Bracket", "Right Bracket"],
};

/** How a word of a key's spelling is said. */
const WORD_WORDS: Readonly<Record<string, string>> = {
  Ctrl: "Control",
  Esc: "Escape",
};

/**
 * **A key's spelling as words**, for a reader to hear where the key is what is shown (the cheat
 * sheet's caps): `⌘⇧↩` is "Command Shift Return", `Ctrl+1–Ctrl+9` "Control 1 to Control 9",
 * `⌘K · F2` "Command K or F2".
 */
export function spokenKeys(said: string, mac: boolean = onAMac()): string {
  const at = mac ? 0 : 1;
  const chord = (one: string) =>
    one
      .split(/(?<=[^+])\+/)
      .flatMap((part) => {
        if (WORD_WORDS[part] !== undefined) return [WORD_WORDS[part]];
        if (part.length > 1 && /^[A-Za-z][A-Za-z0-9]+$/.test(part)) return [part];
        return [...part].map((symbol) => SYMBOL_WORDS[symbol]?.[at] ?? symbol.toUpperCase());
      })
      .join(" ");
  return said
    .split(" · ")
    .map((alternative) =>
      alternative
        .split("–")
        .map((end) => end.split(" ").filter(Boolean).map(chord).join(" "))
        .join(" to "),
    )
    .join(" or ");
}

/**
 * **A row's note without the key its row now shows** (N25): the catalogue's notes say their key
 * in words, as `… ⌘⇧T.`, and where the row draws the key as its `shortcut` the note need not say
 * it again.
 */
export function noteWithoutKey(note: string, key: string): string {
  const said = `${key}.`;
  if (note === said) return "";
  return note.replace(` ${said}`, "").replace(`${said} `, "");
}

/**
 * **Whether `entry` stands back from this keystroke**, by its context rules: what is up in the
 * window, where the keystroke landed, and what its handler says (`held`).
 */
export function yieldsTo(entry: Shortcut, e: KeyboardEvent, held: Held = {}): Yield | undefined {
  const mac = held.mac ?? onAMac();
  const on = e.target;
  return entry.yields.find((rule) => {
    switch (rule) {
      case "modal":
        return held.ownModal !== true && aModalIsUp();
      case "dialog":
        return inADialog();
      case "palette":
        return held.palette === true;
      case "terminal":
        return inAChat(on);
      case "terminal-ctrl":
        return e.ctrlKey && !e.metaKey && inAChat(on);
      case "terminal-off-mac":
        return !mac && inAChat(on);
      case "tab":
        return e.key === "F2" && onARenamingTab(on);
      case "editor":
        return inAnEditor(on);
      case "editor-chords":
        return (
          inAnEditor(on) &&
          (entry.editorKeeps?.[mac ? "mac" : "other"] ?? []).some((chord) => isChord(e, chord))
        );
      case "typing":
        return held.runnable === false && typedInto(on);
      case "menu":
        return inAMenu(on);
    }
    return false;
  });
}

/**
 * **Which of `ids` this keystroke is**, where it applies now: the first whose key it is, unless
 * that one yields here (`yieldsTo`), when it is none of them — the keystroke goes on to whoever
 * has the keyboard. A handler asks for the acts it carries out, in the order it tries them.
 */
export function pressed(
  e: KeyboardEvent,
  ids: readonly string[],
  held: Held = {},
): Shortcut | undefined {
  const mac = held.mac ?? onAMac();
  for (const id of ids) {
    const one = shortcut(id);
    if (one.matches?.(e, mac) !== true) continue;
    return yieldsTo(one, e, { ...held, mac }) === undefined ? one : undefined;
  }
  return undefined;
}

/** Whether this keystroke is `id`'s key, the context aside (a handler on the thing itself). */
export function isKey(e: KeyboardEvent, id: string, mac: boolean = onAMac()): boolean {
  return shortcut(id).matches?.(e, mac) === true;
}

// --- Conflicts -------------------------------------------------------------------------------

/** The places inside another (a dialog holds the palette, a form, a menu, a list's row…). */
const INSIDE: ReadonlyArray<readonly [Scope, Scope]> = [
  ["dialog", "palette"],
  ["dialog", "finder"],
  ["palette", "finder"],
  ["dialog", "form"],
  ["dialog", "menu"],
  ["dialog", "row"],
  ["dialog", "editor"],
  // A view tab holds lists, page tabs, forms and editors, and opens menus and dialogs.
  ["view", "row"],
  ["view", "tab"],
  ["view", "form"],
  ["view", "editor"],
  ["view", "menu"],
  ["view", "dialog"],
];

/** Whether a key in `a` and one in `b` can be pressed in the same place. */
export function overlaps(a: Scope, b: Scope): boolean {
  if (a === b || a === "window" || b === "window") return true;
  return INSIDE.some(([x, y]) => (x === a && y === b) || (x === b && y === a));
}

/** The places a yield stands back in, on this keyboard, for a chord. */
function covers(rule: Yield, platform: Platform, chord: Chord): readonly Scope[] {
  switch (rule) {
    case "modal":
      return ["dialog", "palette", "finder", "form", "menu"];
    case "dialog":
      return ["dialog", "palette", "finder", "menu"];
    case "palette":
      return ["palette", "finder"];
    case "terminal":
      return ["terminal"];
    case "terminal-ctrl":
      return chord.ctrl === true && chord.meta !== true ? ["terminal"] : [];
    case "terminal-off-mac":
      return platform === "other" ? ["terminal"] : [];
    case "tab":
      return ["tab"];
    case "editor":
      return ["editor"];
    case "editor-chords":
      // Only its own chords, which `standsBackFor` reads from the act.
      return [];
    case "typing":
      return ["editor", "form"];
    case "menu":
      return ["menu"];
  }
}

/** Whether `one`, pressed with `chord`, stands back where `other` applies. */
export function standsBackFor(
  one: Shortcut,
  other: Shortcut,
  platform: Platform,
  chord: Chord,
): boolean {
  return one.yields.some((rule) =>
    rule === "editor-chords"
      ? other.where === "editor" &&
        (one.editorKeeps?.[platform] ?? []).some((kept) => chordWord(kept) === chordWord(chord))
      : covers(rule, platform, chord).includes(other.where),
  );
}

/** Whether a keystroke is `chord` exactly: its key, whatever its case, and only its modifiers. */
function isChord(e: KeyboardEvent, chord: Chord): boolean {
  return (
    e.key.toLowerCase() === chord.key.toLowerCase() &&
    e.metaKey === (chord.meta === true) &&
    e.ctrlKey === (chord.ctrl === true) &&
    e.altKey === (chord.alt === true) &&
    e.shiftKey === (chord.shift === true)
  );
}

/** A chord as one word, for comparing: the key (a letter in lower case) and what is held. */
export function chordWord(chord: Chord): string {
  const key = chord.key.length === 1 ? chord.key.toLowerCase() : chord.key;
  const held = [
    chord.ctrl && "ctrl",
    chord.alt && "alt",
    chord.shift && "shift",
    chord.meta && "meta",
  ]
    .filter(Boolean)
    .join("+");
  return held === "" ? key : `${held}+${key}`;
}

/** Two acts on one chord in one place, neither yielding to the other. */
export interface Conflict {
  chord: string;
  platform: Platform;
  ids: [string, string];
}

/**
 * **Every conflict among `entries`**: two acts that declare the same chord on one keyboard, in
 * places that overlap, where neither stands back for the other. The registry test fails on any.
 */
export function conflicts(entries: readonly Shortcut[] = SHORTCUTS): Conflict[] {
  const found: Conflict[] = [];
  for (const platform of ["mac", "other"] as const) {
    entries.forEach((a, at) => {
      for (const b of entries.slice(at + 1)) {
        if (!overlaps(a.where, b.where)) continue;
        for (const chord of a.chords[platform]) {
          const word = chordWord(chord);
          const same = b.chords[platform].find((theirs) => chordWord(theirs) === word);
          if (same === undefined) continue;
          if (standsBackFor(a, b, platform, chord) || standsBackFor(b, a, platform, same)) continue;
          found.push({ chord: word, platform, ids: [a.id, b.id] });
        }
      }
    });
  }
  return found;
}

/** A keystroke for `chord`, as a browser would deliver it, for the tests and the stories. */
export function keystroke(chord: Chord, target?: EventTarget): KeyboardEvent {
  const event = new KeyboardEvent("keydown", {
    key: chord.key,
    code: chord.code ?? codeOf(chord.key),
    metaKey: chord.meta === true,
    ctrlKey: chord.ctrl === true,
    altKey: chord.alt === true,
    shiftKey: chord.shift === true,
    bubbles: true,
    cancelable: true,
  });
  if (target !== undefined) Object.defineProperty(event, "target", { value: target });
  return event;
}

/** The physical key a US keyboard types `key` with, where it is a letter or a digit. */
function codeOf(key: string): string {
  if (/^[a-z]$/i.test(key)) return `Key${key.toUpperCase()}`;
  if (/^[0-9]$/.test(key)) return `Digit${key}`;
  return "";
}
