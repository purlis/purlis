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
 *   (always, for its Ctrl chords, or off a Mac only), a view off a Mac (its Back and Forward), a
 *   tab that renames on the key, an editor, a field it would be typed into. A key that yields does nothing there: the event goes on to whoever
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
import { paneStepOf, paneStepSaid, tabStepOf, tabStepSaid } from "../moveKeys";
import { f6Of } from "../regionCycle";
import { opensAShell, shellKeySaid } from "../shellKey";
import { sizeKey, type SizeKey } from "../textSize";
import { isRevertKey, revertKeySaid } from "../themeDrafts";
import { submits, submitKeySaid } from "../ui/field/submitKey";
import { asksForAMenu } from "../ui/menu/ContextMenu";
import { asksForTheMenu } from "../terminalRows";
import { fileKeyOf, fileKeySaid, opensIt } from "./paletteKeys";
import { opensFind, opensFindInAFile } from "./findKeys";
import { copiesALink, linkCopyKeySaid, linksListKeySaid, opensTheLinksList } from "../paneLinksKey";
import { viewHistoryKeyOf } from "./viewKeys";
import { focusKeySaid, opensTheShortcuts, shortcutsKeySaid, togglesFocus } from "./ownKeys";
import { TRIAGE_LETTER, triageKeyOf, triageKeySaid, type TriageKey } from "../asksTriage";
import {
  aModalIsUp,
  inADialog,
  inAChat,
  inAMenu,
  inAnEditor,
  inAView,
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
  /** Off a Mac, the keystroke landed in a view, whose Back and Forward are Alt and an arrow. */
  | "view-off-mac"
  /** The keystroke landed on a tab, or a chat's row, that answers the key itself (`F2`
   *  renames it). */
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
/** The four arrows, for the pane keys (`moveKeys.ts`). */
const ARROW_KEYS = ["ArrowLeft", "ArrowUp", "ArrowRight", "ArrowDown"];
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
/** Links in this pane…'s chords (#6220): ⇧⌘L, and Ctrl+Shift+L off a Mac. */
const PANE_LINKS = { mac: cmd("L", { shift: true }), other: ctl("L", { shift: true }) };
/** No modifier held. */
const plain = (e: KeyboardEvent) => !e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey;
/** Asks' triage keys (`asksTriage.ts`), each with what the cheat sheet says it does. */
const TRIAGE_ACTS: readonly (readonly [TriageKey, string])[] = [
  ["later", "Put the ask off for later: on to the next"],
  ["go", "Go to the ask's chat"],
  ["snooze", "Snooze the ask's chat"],
  ["stop", "Stop the ask's chat"],
];

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
    // Ctrl+K is a chat's kill-line; a tab and a chat's row rename on F2; and in a code editor the editor
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
    id: "view.asks",
    does: "Show Asks",
    group: "Window",
    where: "window",
    kind: "key",
    chords: { mac: [cmd("I", { shift: true })], other: [ctl("I", { shift: true })] },
    said: sideSaid("asks"),
    yields: ["dialog"],
    rows: ["view.show:asks"],
    matches: isSide({ show: "asks" }),
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
    does: "Focus this pane: zoom in a split, focus mode in one pane",
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
    does: "Open a shell",
    group: "Tabs and chats",
    where: "window",
    kind: "key",
    chords: { mac: [cmd("T", { shift: true })], other: [ctl("T", { shift: true })] },
    said: shellKeySaid,
    yields: ["modal"],
    rows: ["shell.new"],
    matches: opensAShell,
  },
  // **⌘T is an empty tab and ⌘N a new chat** (#5292; spec #5288 story 82), so the keys say what
  // they do: the strip's `+` and its list's second row. ⌘⇧T stays New shell (story 83). xterm.js
  // sends nothing for a ⌘ chord, so a Mac's chat loses neither. Off a Mac, Ctrl+T and Ctrl+N
  // are a shell's transpose and next line, so a chat's terminal keeps them there, as it keeps
  // ⌘1–⌘9's Ctrl chords.
  {
    id: "tab.new",
    does: "Open an empty tab",
    group: "Tabs and chats",
    where: "window",
    kind: "key",
    chords: { mac: [cmd("t")], other: [ctl("t")] },
    said: (mac) => (mac ? "⌘T" : "Ctrl+T"),
    yields: ["modal", "palette", "terminal-off-mac"],
    rows: ["tab.new"],
    matches: (e, mac) => isChord(e, mac ? cmd("t") : ctl("t")),
  },
  {
    id: "chat.new",
    does: "Start a new chat",
    group: "Tabs and chats",
    where: "window",
    kind: "key",
    chords: { mac: [cmd("n")], other: [ctl("n")] },
    said: (mac) => (mac ? "⌘N" : "Ctrl+N"),
    yields: ["modal", "palette", "terminal-off-mac"],
    rows: ["chat.new"],
    matches: (e, mac) => isChord(e, mac ? cmd("n") : ctl("n")),
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
    // Each chat's Use skill… row (`chat.useskill:<session>`) says it, as each tab's Close row says
    // ⌫: the key opens it on the chat in front, the pane with the keyboard in a split tab.
    rows: ["chat.useskill:"],
    matches: (e, mac) => isChord(e, mac ? USE_SKILL.mac : USE_SKILL.other),
  },
  {
    id: "tasks.menu",
    does: "The focused chat's tasks",
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
    id: "pane.back",
    does: "Back to the chat this pane showed before",
    group: "Tabs and chats",
    where: "window",
    kind: "key",
    chords: { mac: [cmd("H", { shift: true })], other: [ctl("H", { shift: true })] },
    said: (mac) => taskKeySaid("back", mac),
    yields: ["dialog", "typing"],
    rows: [TASK_KEY_ROW.back],
    matches: isTask("back"),
  },
  {
    id: "pane.focus",
    does: "Move to the pane in that direction",
    group: "Tabs and chats",
    where: "window",
    kind: "key",
    chords: {
      mac: ARROW_KEYS.map((key) => cmd(key, { alt: true })),
      other: ARROW_KEYS.map((key) => bare(key, { alt: true })),
    },
    said: paneStepSaid,
    // Off a Mac Alt and an arrow is a byte to a terminal (word movement), a view's Back and
    // Forward (`moveKeys.ts`), and an editor's own (CodeMirror moves a line on Alt with Up and
    // Down, and by syntax on Alt with Left and Right). On a Mac an editor adds a cursor above or
    // below on ⌘⌥ with Up and Down, and keeps those two.
    yields: ["dialog", "menu", "terminal-off-mac", "view-off-mac", "editor-chords"],
    editorKeeps: {
      mac: ["ArrowUp", "ArrowDown"].map((key) => cmd(key, { alt: true })),
      other: ARROW_KEYS.map((key) => bare(key, { alt: true })),
    },
    matches: (e, mac) => paneStepOf(e, mac) !== undefined,
  },
  {
    id: "tab.next",
    does: "The next tab",
    group: "Tabs and chats",
    where: "window",
    kind: "key",
    chords: {
      mac: [cmd("]", { shift: true, code: "BracketRight" })],
      other: [ctl("]", { shift: true, code: "BracketRight" })],
    },
    said: (mac) => tabStepSaid("next", mac),
    yields: ["dialog", "typing"],
    matches: (e, mac) => tabStepOf(e, mac) === "next",
  },
  {
    id: "tab.previous",
    does: "The previous tab",
    group: "Tabs and chats",
    where: "window",
    kind: "key",
    chords: {
      mac: [cmd("[", { shift: true, code: "BracketLeft" })],
      other: [ctl("[", { shift: true, code: "BracketLeft" })],
    },
    said: (mac) => tabStepSaid("previous", mac),
    yields: ["dialog", "typing"],
    matches: (e, mac) => tabStepOf(e, mac) === "previous",
  },
  {
    id: "layout.undo",
    does: "Undo the last layout change",
    group: "Tabs and chats",
    where: "window",
    kind: "key",
    chords: { mac: [cmd("z")], other: [ctl("z")] },
    said: (mac) => (mac ? "⌘Z" : "Ctrl+Z"),
    // A field's and an editor's own Undo come first, and off a Mac Ctrl+Z is the chat's
    // (#4763): its handler holds it as not runnable in a field, so the field keeps the key.
    yields: ["modal", "dialog", "palette", "editor", "terminal-off-mac", "typing"],
    // The palette's Undo row (`UNDO_LAYOUT_ID`) runs the same Undo.
    rows: ["layout.undo"],
    matches: (e, mac) => isChord(e, mac ? cmd("z") : ctl("z")),
  },
  {
    id: "pane.move",
    does: "Pick up the focused pane, chat or tab to move it with the keys",
    group: "Tabs and chats",
    where: "window",
    kind: "key",
    chords: { mac: [cmd("M", { shift: true })], other: [ctl("M", { shift: true })] },
    said: (mac) => (mac ? "⌘⇧M" : "Ctrl+Shift+M"),
    // **Placement mode** (#4764): claimed over a chat's terminal, where xterm.js sends no byte
    // for it on either keyboard, so nothing is kept from a chat; a dialog keeps its keys.
    yields: ["modal", "dialog", "palette", "typing"],
    matches: (e, mac) => isChord(e, mac ? cmd("M", { shift: true }) : ctl("M", { shift: true })),
  },
  {
    id: "tab.rename",
    does: "Rename the focused tab, or its one chat",
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
    id: "row.rename",
    does: "Rename the focused chat in the list",
    group: "Tabs and chats",
    where: "row",
    kind: "key",
    chords: both(bare("F2")),
    said: says("F2"),
    yields: [],
    rows: ["chat.rename:"],
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
    id: "pane.links",
    does: "The links on the pane's screen: Links in this pane…",
    group: "Tabs and chats",
    where: "terminal",
    kind: "key",
    // `L` for links (#6220, D5). ⇧⌘L was no key of the window's; Ctrl with Shift and a letter is
    // no byte to a terminal's program, so it is the pane's off a Mac, as Ctrl+Shift+F is.
    chords: { mac: [PANE_LINKS.mac], other: [PANE_LINKS.other] },
    said: linksListKeySaid,
    // The terminal's menu row that opens the list (#6219) says this key, drawn and to a reader.
    rows: ["terminal.links"],
    yields: [],
    matches: opensTheLinksList,
  },
  {
    id: "links.copy",
    does: "Copy the target of the link a Links in this pane… row is on",
    group: "Dialogs and forms",
    where: "row",
    kind: "key",
    // As Copy anywhere; Shift and Alt up, so ⌘⇧C and ⌥⌘C stay what else has them (#6220).
    chords: { mac: [cmd("c")], other: [ctl("c")] },
    said: linkCopyKeySaid,
    yields: [],
    matches: copiesALink,
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
  // **Keyboard triage in Asks** (ADR 0121 decision 9, #6016): on an ask's row, each presses the
  // row's own button, so the settle guard decides it as it decides a click. Never a letter
  // alone (I-11), and never from a reply box, whose letters are typed.
  ...TRIAGE_ACTS.map(([key, does]): Shortcut => ({
    id: `asks.${key}`,
    does,
    group: "Tabs and chats",
    where: "row",
    kind: "key",
    chords: {
      mac: [cmd(TRIAGE_LETTER[key].toLowerCase(), { alt: true, code: `Key${TRIAGE_LETTER[key]}` })],
      other: [
        bare(TRIAGE_LETTER[key], { alt: true, shift: true, code: `Key${TRIAGE_LETTER[key]}` }),
      ],
    },
    said: (mac) => triageKeySaid(key, mac),
    yields: ["modal", "dialog", "menu", "typing"],
    matches: (e, mac) => triageKeyOf(e, mac) === key,
  })),
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
    matches: (e) => e.key === "Enter" && !e.altKey,
  },
  {
    // **Open beside has no chord of its own** (#4769, the orchestrator's ruling on #4699): ⌥ on
    // the press that opens a view, which on a palette row is ⌥Enter (Alt+Enter off a Mac).
    id: "palette.beside",
    does: "Open the view aimed at beside the focused pane",
    group: "Palette",
    where: "palette",
    kind: "key",
    chords: both(bare("Enter", { alt: true })),
    said: (mac) => (mac ? "⌥↩" : "Alt+Enter"),
    yields: [],
    matches: (e) => e.key === "Enter" && e.altKey,
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
  "view.asks",
];

/** Asks' triage keys (#6016): one handler, on the list, carries them out. */
export const TRIAGE_KEY_IDS: readonly string[] = TRIAGE_ACTS.map(([key]) => `asks.${key}`);

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

/**
 * **The key a menu's row shows, and the same key as a reader is told it** (#6043): what the
 * menu family's `MenuShortcuts` answers, so a row draws the one and says the other by its
 * `aria-keyshortcuts`, and the two never come from different rows of the registry.
 */
export function menuRowKey(
  row: string,
  mac: boolean = onAMac(),
): { said: string; aria?: string } | undefined {
  const said = rowKey(row, mac);
  return said === undefined ? undefined : { said, aria: rowAriaKeys(row, mac) };
}

/** A modifier as a key's spelling writes it, by the name `aria-keyshortcuts` gives it. */
const ARIA_MODIFIERS: Readonly<Record<string, string>> = {
  "⌘": "Meta",
  "⇧": "Shift",
  "⌥": "Alt",
  "⌃": "Control",
  Cmd: "Meta",
  Command: "Meta",
  Ctrl: "Control",
  Control: "Control",
  Shift: "Shift",
  Alt: "Alt",
  Option: "Alt",
};

/** A key's symbol as its spelling draws it, by its UI Events value. */
const ARIA_SYMBOLS: Readonly<Record<string, string>> = {
  "↩": "Enter",
  "⌫": "Backspace",
  "↑": "ArrowUp",
  "↓": "ArrowDown",
  "←": "ArrowLeft",
  "→": "ArrowRight",
};

/** The order `ariaChord` names modifiers in. */
const MODIFIER_ORDER = ["Control", "Alt", "Shift", "Meta"];

/**
 * **A key as a row draws it, named for `aria-keyshortcuts`** (#6043): a Mac's symbols (`⌘⇧T`,
 * `⌫`) or another keyboard's words (`Ctrl+Shift+T`, `Delete`), alternatives apart by ` · `.
 * For a row whose caller knows its key and no registry entry names it (`keySaid`, a terminal's
 * `⌘C`), so a reader hears the key the row draws. A range or a sequence of chords is no one
 * shortcut, and names nothing.
 */
export function ariaKeysOfSaid(said: string): string | undefined {
  const chords = said.split(" · ").map((one) => ariaOfOneChord(one.trim()));
  return chords.length === 0 || chords.some((one) => one === undefined)
    ? undefined
    : chords.join(" ");
}

function ariaOfOneChord(said: string): string | undefined {
  if (said === "" || said.includes(" ") || said.includes("–")) return undefined;
  const modifiers = new Set<string>();
  let key: string;
  if (said.length > 1 && said.includes("+")) {
    const parts = said.split("+");
    key = parts.pop() ?? "";
    for (const part of parts) {
      const modifier = ARIA_MODIFIERS[part];
      if (modifier === undefined) return undefined;
      modifiers.add(modifier);
    }
  } else {
    const symbols = [...said];
    while (symbols.length > 1 && "⌘⇧⌥⌃".includes(symbols[0]))
      modifiers.add(ARIA_MODIFIERS[symbols.shift() as string]);
    key = symbols.join("");
  }
  if (key === "") return undefined;
  key = ARIA_SYMBOLS[key] ?? (key.length === 1 ? key.toUpperCase() : key);
  return [...MODIFIER_ORDER.filter((one) => modifiers.has(one)), key].join("+");
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
      case "view-off-mac":
        return !mac && inAView(on);
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
    case "view-off-mac":
      return platform === "other" ? ["view"] : [];
    case "tab":
      // A chat's row renames on F2 too (#5343), marked as a tab is (`RENAMES_ON_F2`).
      return ["tab", "row"];
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
