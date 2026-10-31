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
  pinned: "Pinned",
  settings: "Settings…",
  saving: "Saving…",
  linkWorkItem: "Link to work item…",
  unlinkWorkItem: "Unlink work item",
  cancelSmartClose: "Cancel smart close",
  brief: "Brief",
  answer: "Answer its question",
  activity: "Activity",
  network: "Network",
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
  shellHere: "Open a shell tab here",
  openInEditor: "Open in your editor",
  startAChatHere: "Start a chat here",
  addToChat: "Add to a chat's context",
  removeFolder: "Remove folder",
  discardAnyway: "Discard that work and remove anyway",
  clone: "Clone",
  dropFromWorkspace: "Remove from workspace…",
  // A secret's row in a vault's tab (`VaultTab.tsx`).
  editValue: "Edit value…",
  copy: "Copy",
} as const;

/** The rows that open a submenu, and what the submenu's own tooltip says. */
export const SUBMENU = {
  open: { said: "Open", tip: "Each opens in a tab of its own." },
  ask: {
    said: "Ask",
    tip: "A task starts as the persona you pick, and its report comes back to this chat.",
  },
  stop: { said: "Stop", tip: "Ways to stop this chat, or the tasks it asked for." },
  curate: { said: "Curate", tip: undefined },
  moveTo: { said: "Move to", tip: undefined },
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
