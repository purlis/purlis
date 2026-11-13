// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a menu's rows say**, each written once (`docs/ui-primitives.md`, "Menus").
 *
 * A menu is about the thing it was opened on, so a row is a short verb and does not say that
 * thing's name again: *Rename…*, not *Rename chat console-developer 16…*. The catalogue's title
 * (`actions.ts`) still names it, for the palette, where a row stands among every other row of
 * the window. `menuModel.ts` gives each catalogue row its words from here, and the menu-only
 * builders of `actions.ts` (`viewRows`, `fileRows`) read them from here too.
 *
 * **Nothing is imported here**, so the end-to-end specs read the same words the window draws
 * (`e2e/menus.ts`), and a rename is one edit.
 */
export const SAID = {
  switchTo: "Switch to",
  focus: "Focus",
  rename: "Rename…",
  // A chat's own (#5343): said with what it renames, so a pane's ⋯ never reads like its tab's.
  renameChat: "Rename chat…",
  // A split tab's (#5343): it names the tab, never a chat.
  renameTab: "Rename tab…",
  pinned: "Pinned",
  // A pin's place among the title bar's chips (#3128).
  moveLeft: "Move left",
  moveRight: "Move right",
  settings: "Settings…",
  saving: "Saving…",
  linkWorkItem: "Link to work item…",
  unlinkWorkItem: "Unlink work item",
  cancelSmartClose: "Cancel smart close",
  useSkill: "Use skill…",
  // A pane's links (#6220, #6332): the list of what is on its screen, from every way in.
  linksInThisPane: "Links in this pane…",
  ignore: "Ignore until it asks again",
  end: "End…",
  keepOpen: "Keep open",
  tileAll: "Tile all",
  move: "Move…",
  closeOtherTabs: "Close other tabs",
  thisMachine: "This machine",
  brief: "Brief",
  answer: "Answer its question",
  // A pane head's one state button (#4760, spec #4750 story 55): its row's act in a word, for a
  // button whose name is the row's whole title.
  answerButton: "Answer",
  stopButton: "Stop",
  skills: "Skills",
  openInStudio: "Open in the Skill studio",
  activity: "Activity",
  network: "Network",
  chatLog: "Chat log",
  restart: "Restart",
  restartMidTurn: "Restart when this turn ends",
  startFresh: "Start fresh",
  stopAllTasks: "Stop all tasks",
  taskStop: "Stop and get its report",
  taskClose: "Close now",
  show: "Show",
  ownTab: "Move to its own tab",
  newShell: "New shell",
  newChat: "New chat",
  newTab: "New tab",
  newBranch: "New branch…",
  startChatsHere: "Start new chats here",
  toNewWindow: "Move to a new window",
  // A tab's and a pane's (#4768), in the ruling's words: into a tab window of the project.
  tabToNewWindow: "Move to new window",
  toMainWindow: "Move to the main window",
  // A view's icon (`actions.ts`, `viewRows`): to its half of the Panel, or back to its sidebar.
  toPanel: "Move to panel",
  toSidebar: "Move to sidebar",
  close: "Close",
  open: "Open",
  edit: "Edit",
  editDefinition: "Edit persona.md",
  setProfile: "Set profile…",
  delete: "Delete",
  deleteAsks: "Delete…",
  markDone: "Mark done",
  reopen: "Reopen",
  forget: "Forget",
  resume: "Resume",
  browseFiles: "Browse files",
  // A file's or folder's rows, and a branch's own folder's (`actions.ts`, `fileRows`).
  copyRelativePath: "Copy relative path",
  copyAbsolutePath: "Copy absolute path",
  shellHere: "Open a shell here",
  openInEditor: "Open in your editor",
  // The person's own workspace files' writes (#5376): each asks first, so each has its ellipsis.
  newFile: "New file…",
  newFolder: "New folder…",
  importFiles: "Import files…",
  startAChatHere: "Start a chat here",
  addToChat: "Add to a chat's context",
  // A LIVE workspace's own file or folder (#5377).
  saveWithTheProject: "Save with the project",
  // An .html file's row and preview (#5953): the team's pin, never the private Pin of a tab, so
  // it names the workspace and asks first.
  pinToWorkspace: "Pin to workspace…",
  removeFolder: "Remove folder",
  discardAnyway: "Discard that work and remove anyway",
  // A chat's Branch ▸ (#4851): the words of `command-table.md` rows 24 and 25.
  mergeItsBranch: "Merge its branch",
  removeItsBranchFolder: "Remove its branch folder",
  clone: "Clone",
  dropFromWorkspace: "Remove from workspace…",
  dismiss: "Dismiss",
  // A secret's row in a vault's tab (`VaultTab.tsx`).
  editValue: "Edit value…",
  copy: "Copy",
  // Named by the core's refusal of a fill (`secrets/no_seed.rs`, MARK_WORDS and UNMARK_WORDS).
  markNoSeed: "Mark as no 2FA seed…",
  unmarkNoSeed: "Remove the no-seed mark",
} as const;

/** The rows that open a submenu, and what the submenu's own tooltip says. */
export const SUBMENU = {
  open: { said: "Open", tip: "Each shows something of this chat, and changes nothing it runs." },
  ask: {
    said: "Ask",
    tip: "A task starts as the persona you pick, and its report comes back to this chat.",
  },
  stop: { said: "Stop", tip: "Ways to stop this chat, or the tasks it asked for." },
  curate: { said: "Curate", tip: undefined },
  moveTo: { said: "Move to", tip: undefined },
  toWindow: {
    said: "Move to window",
    tip: "Each moves it into that window of this project, as a tab of its own. Nothing ends.",
  },
  branch: { said: "Branch", tip: "Each acts on the branch folder purlis cut for this chat." },
  // A vault's secret row (#5240): a row per persona that uses the vault, or names none, and
  // does not declare the key.
  offerNeed: {
    said: "Offer as a Secret need",
    tip: "Each shows the line adding this key to that persona's Secret needs, for you to edit and add.",
  },
} as const;

/**
 * **What a row that cannot be undone costs, under its words.** The catalogue's note says the
 * whole of it and is the row's tooltip and description; this is the part that changes the
 * decision, in a short sentence or two. It is never cut: where a menu is too narrow for it, it
 * wraps. Each says no more than the catalogue's note for its row says.
 */
export const WARNS = {
  ends: "Ends the program it runs. There is no undo.",
  fresh: "Starts again with no conversation.",
  stops: "Ends the chat.",
  stopsBelow: "Ends it and every task below it.",
  stopsTasks: "Ends its tasks still at work. The chat keeps running.",
  taskStops: "Ends the task after its report.",
  taskCloses: "Ends the task at once, with no report.",
  closesProject: "Ends every chat in it.",
  deletesWorkspace: "Deletes its clones, memory and todos. There is no undo.",
  deletesPersona: "Deletes its definition, memory and refs.",
  deletesVault: "A keychain vault's secrets cannot be recovered.",
  forgets: "Drops it with nothing journalled.",
  deletesDone: "Deletes it with nothing journalled. There is no undo.",
} as const;
