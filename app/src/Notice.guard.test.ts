// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync, readdirSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * **Every standing line in the window is a Notice** (NO-1 #1223, rulings V91b, V91d, V91q).
 *
 * A Notice is a standing line about something true now (CONTEXT.md), and `Notice.tsx` is the
 * one way to draw one: its props will not take a line with no way out. Before it, each line was
 * a `<p className="came-back" role="status">` written by hand, and about thirty of the window's
 * messages left the operator reading a problem with nothing to press. This fails when one is
 * built that way again:
 *
 * - **a hand-built standing line**: any live region (a `status` or `alert` role, one picked by
 *   an expression, or `aria-live`) in any source file under `src/`, whatever its class. The
 *   ones that are not Notices (a dialog's refusal, a live count) are listed in `NOT_NOTICES`,
 *   each with why, by exact count per file, so a new one fails and a removed one is crossed off;
 * - **a hand-built way out**: a `dismiss` or `offer` button class, or a `notice*` class, named
 *   anywhere but `Notice.tsx`, or a stylesheet rule that draws `dismiss` or `offer`.
 *
 * And it **lists the debt** (V91q): every Notice whose only remedy is Copy command, a command to
 * type somewhere else. The list is `COPY_ONLY`, exactly, so adding to it is a visible change.
 *
 * Read as text, as `settings/oldFormClasses.test.ts` (DS-3e) and `paint.test.ts` read theirs:
 * jsdom computes no stylesheet, and a rendered window shows only the lines its test set up.
 */

/*
 * **Why a live region outside a Notice has no link or fix of its own: NO-8's checklist**
 * (#1233, V91n). Each dialog or tab refusal was looked at for a cheap way out, and the entries
 * below say what each got, or which of these reasons it has none for.
 */
/** The refusal of the action just pressed, in the core's sentence, beside the button that
 *  pressed it: pressing again is the retry, with what was typed kept. */
const ACTION =
  "a dialog's or tab's refusal of the action just pressed: an inline error, not a Notice " +
  "(V91n). No cheap link (NO-8): the core's sentence names the cause, and the button retries";
/** A warning a dialog asks over: its own buttons are the way out. */
const WARNING =
  "a dialog's warning about what its button would do: its own Cancel and confirm are the way out";
/** A read refusal in a tab that reads again when the project changes on disk. */
const HEALS =
  "a view tab's read refusal, inside the tab. No link needed (NO-8): the tab reads again " +
  "whenever the project changes on disk, so mending the file clears it";
/** A read refusal read once: Read again is a follow-up on NO-8's list. */
const READ_ONCE =
  "a read refusal, inside its tab or dialog. Read again is NO-8's follow-up: it is read once " +
  "when it opens, and reopening it is the retry today";
/** A vault's read, health and write refusals. */
const VAULT =
  "a vault tab's read, health and write refusals: the provider's own sentence names what to " +
  "do (sign in, unlock), and the vault's box has Read again; a write's refusal is ACTION's case";
/** An extension view's refusals. */
const VIEWS =
  "an extension view's refusals: its Refresh reads again, and the view is the extension's, so " +
  "charter has no fix of its own to offer (NO-8)";

/** A line said while something is read, or that it could not be: progress and a footnote to the
 *  surface's own answer, not something true now with a way out (#1719, lane KC's lines). */
const READING =
  "a line said while the surface reads what it shows, or that the read failed and what the " +
  "surface offers instead: a screen reader is told as the eye is, and the surface's own " +
  "controls still work, so it has no way out of its own (#1719)";

/**
 * **Every live region that is not a Notice, with why. Exact counts per file**: a new one in any
 * file fails until it is a Notice or is listed here with its reason, and a removed one is
 * crossed off. A refusal inside a dialog stays an inline error (V91n), and a live value (a count,
 * progress) is not a standing line.
 */
const NOT_NOTICES: Record<string, { count: number; why: string }> = {
  "About.tsx": {
    count: 1,
    why:
      "a dialog's read refusal: an inline error, not a Notice (V91n). Read again beside it is " +
      "the retry (NO-8's follow-up, #1296)",
  },
  "ActivityTab.tsx": {
    count: 2,
    why:
      "on a question's line (#1496): that the chat to answer in could not be asked for, in the " +
      "core's sentence beside Answer, which is the retry; and that the person's answer was " +
      "sent, said once for whoever cannot see the Answer control go. Neither stands for " +
      "something true now that has a way out of its own",
  },
  "AddDockerPool.tsx": {
    count: 2,
    why:
      ACTION +
      ". Add a Docker pool's refusal (#2679), and a cluster pool's (#3292), each in the core's " +
      "words, under the form it refused; the form, kept as it was typed, and Add pool or Check " +
      "its key are the way on",
  },
  "ui/notice/Notice.tsx": {
    count: 1,
    why:
      "the notice family's message part, the live region of a card Notice (#3344): drawn live " +
      "only by `Notice.tsx`, which this guard holds as the one way to draw a standing line",
  },
  "AddVision.tsx": {
    count: 1,
    why: ACTION,
  },
  "AnswerQuestion.tsx": {
    count: 2,
    why:
      "the refusal of the person's answer to a task's question (#1496), in the core's sentence " +
      "under the box, with what was typed kept: Send is the retry. And, where the form was " +
      "opened for a task that asks nothing now, the sentence that says so, beside Close",
  },
  "AskPersona.tsx": {
    count: 1,
    why: ACTION,
  },
  "InspectorView.tsx": {
    count: 1,
    why:
      "a DevTools view's bar (#2028): that the person drives the browser while DevTools are " +
      "open, drawn by purlis outside the frame, with a refusal of their input said in it, " +
      "replaced by the next; Done on the browser closes them",
  },
  "BrowserView.tsx": {
    count: 1,
    why:
      "a browser view's bar (#2014): who drives the page under it, drawn by purlis outside the " +
      "frame, said to a screen reader as it changes. Done beside it hands the browser back, and " +
      "a refusal of the person's input is said in it, replaced by the next",
  },
  "Brief.tsx": {
    count: 3,
    why:
      READ_ONCE +
      "; and what Copy did in the Brief panel, beside its button: replaced by the next press, " +
      "not about something true now (#1494); and " +
      READING +
      ": reading the brief",
  },
  "ChangesView.tsx": {
    count: 2,
    why:
      "the repos bar's read refusal and a branch it cannot read: the Changes view has nothing " +
      "to press (ADR 0038), so their way out is the explorer's Read again on the same read (NO-4)",
  },
  "ChangeActions.tsx": {
    count: 4,
    why: ACTION,
  },
  "AddToAChat.tsx": {
    count: 1,
    why: "a filter that matches no chat says so where the list was, as the list's empty line (#2910)",
  },
  "ui/empty-state/EmptyState.tsx": {
    count: 1,
    why:
      'the role a caller passes (`role="status"`) for an empty state that is a surface\'s ' +
      "answer, not a standing line: each caller is listed here by its own count",
  },
  "ui/card/Totals.tsx": {
    count: 1,
    why: "live totals' counts, said politely as they change, at most once a second (#3673): a live value, not a line",
  },
  "ui/meter/Meter.tsx": {
    count: 1,
    why: "a progress bar's value, said politely as it changes, at most once a second (#3674): a live value, not a line",
  },
  "Cockpit.tsx": {
    count: 1,
    why: "a branch's apart count, said as it changes: a live value, not a line",
  },
  "ChatAsk.tsx": {
    count: 1,
    why: ACTION,
  },
  "ChatsSection.tsx": {
    count: 3,
    why:
      "the Chats list's live count of what its filter hides, and why the key just pressed on " +
      "a row did nothing: a live value and an action's answer, replaced by the next (#1499); " +
      "and what a restart asked for from a row of a chat with no pane says, refused or " +
      "waiting, the core's answer to that press, gone when the restart is (#1462)",
  },
  "DeleteVault.tsx": {
    count: 2,
    why: ACTION,
  },
  "DeleteWorkspace.tsx": {
    count: 2,
    why: ACTION,
  },
  "Doctor.tsx": {
    count: 2,
    why:
      "the Doctor dialog's own refusal, and what its last Fix changed or why it was refused " +
      "(FX-1): inside the dialog, over the rows it changed",
  },
  "Explorer.tsx": {
    count: 3,
    why:
      "a folder of a branch's tree that could not be read, in the tree where its entries go: a " +
      "row's own refusal, and opening the folder again reads it again (NO-4); a branch the " +
      "filter could not find files in by name, said where the files found go, and typing again " +
      "asks again (#3343); and how many file rows are picked, a live count read out as it " +
      "changes (#1151)",
  },
  "ExtensionAction.tsx": {
    count: 1,
    why: ACTION,
  },
  "Extensions.tsx": {
    count: 4,
    why:
      "an installed extension's row trouble, a colour theme's complaint (#2321) and an icon " +
      "theme's complaint, inside its row of the Extensions dialog: the row's Review and its box " +
      "are the way out (#630); and the filter's line where it matches no extension (#2910)",
  },
  "FindBar.tsx": { count: 1, why: "the find bar's match count, a live value" },
  "FinishedTasks.tsx": {
    count: 1,
    why:
      ACTION +
      ": a finished task that could not be reopened, on its row beside Reopen. The core's " +
      "sentence says why (its folder is gone, its harness named no conversation), and its " +
      "report is still on the row to read (#1485)",
  },
  "FirstRun.tsx": {
    count: 4,
    why:
      "the first-run page's progress and refusal, inside its tab; and " +
      READING +
      ": reading what this machine has, or that it could not",
  },
  "MakeItAProject.tsx": {
    count: 2,
    why:
      ACTION +
      ". The offer to make a folder a project (#1822): the project being made, and the core's " +
      "refusal of it; its Make it a project and Not now are the way out",
  },
  "ForgeSignIn.tsx": {
    count: 4,
    why:
      ACTION +
      ": a forge sign-in that could not open the project its shell tab goes in; the account a " +
      "token just signed in to, said once (#1814); and " +
      READING +
      ": reading which forge tools this machine has, or that it could not",
  },
  "ForgeDeviceSignIn.tsx": {
    count: 4,
    why:
      ACTION +
      ". The browser sign-in's own lines (#729), inside its dialog: the code being asked for, " +
      "that the code was copied, the wait for the person to approve it on the forge's page, and " +
      "the core's sentence for how the flow ended; its Try again and Cancel are the way out",
  },
  "ForgeGitLabSignIn.tsx": {
    count: 2,
    why:
      ACTION +
      ". The browser sign-in's own lines (#730), inside its dialog: that it is waiting for the " +
      "browser, and the core's refusal of the sign-in; its Try again and Cancel are the way out",
  },
  "ForgeTokenSignIn.tsx": {
    count: 2,
    why:
      ACTION +
      ". The token sign-in's own lines (#1814): the scopes the host needs, re-read as the host " +
      "is typed, and the core's refusal of the token; its Sign in and Cancel are the way out",
  },
  "FirstTaskTab.tsx": {
    count: 3,
    why:
      ACTION +
      ". Its read of the start options is a Notice with Read again (#1296). And " +
      READING +
      ": the harnesses it could not look at and the branches it could not list",
  },
  "CreateOnForge.tsx": {
    count: 6,
    why:
      ACTION +
      ": a creation that failed, with Retry beside it and what was taken back (#1819); a name " +
      "that cannot be one; the owners the forge could not list, with Retry; the folder dialog " +
      "Choose a folder… could not open; the owners being asked for; and the creation's stages " +
      "while it runs, with Cancel, said as they change",
  },
  "PublishProject.tsx": {
    count: 3,
    why:
      ACTION +
      ": a publish that failed, with Retry beside it, what was taken back and, for a repo the " +
      "forge kept, a link to its settings (#1820); the publish's stages while it runs, with " +
      "Cancel, said as they change; and where it was published, inside the dialog that did it",
  },
  "GetProject.tsx": {
    count: 3,
    why:
      ACTION +
      ": a clone that could not get the project, with Retry beside it (#1815); and the folder " +
      "dialog its Open a folder… or Choose a folder… could not open (#1291); and the clone's " +
      "progress while it runs, with Cancel, said as it changes",
  },
  "ImportCliLogin.tsx": {
    count: 2,
    why:
      ACTION +
      ". The offer to use an existing gh or glab login (#1376): the import in progress, and the " +
      "core's refusal of it beside the offer; its Use my login and Not now are the way out",
  },
  "GitMissing.tsx": {
    count: 3,
    why:
      ACTION +
      ". The git screen's own lines (#1813): Install's or the look's refusal, that Apple's " +
      "installer is open, and the look in progress; its Install, Check again and Not now are " +
      "the way out",
  },
  "HarnessSetupTab.tsx": {
    count: 1,
    why: ACTION,
  },
  "Inbox.tsx": {
    count: 3,
    why:
      ACTION +
      ". An ask's answer, or a reply, the source refused: said in that ask's own row, which stays (#1692); a grant pressed on an update that just moved, said in that update's row (#1693); a way out of a granting Notice pressed just after it was drawn or moved, said at the top of the Inbox's Notices (#1695)",
  },
  "KillSwitch.tsx": {
    count: 1,
    why: "the kill switch's refusal, on the title bar control it belongs to",
  },
  "LinkWorkItem.tsx": {
    count: 1,
    why: ACTION,
  },
  "LiveDialog.tsx": {
    count: 4,
    why:
      ACTION +
      ". A switch whose save did not happen offers Go to Saving, where the save is mended " +
      "(NO-8's follow-up, #1296). And " +
      READING +
      ": reading the workspace, and asking the forge who can read its repo",
  },
  "MemoryArchiveTab.tsx": {
    count: 3,
    why: HEALS,
  },
  "MemoryTab.tsx": {
    count: 4,
    why: HEALS,
  },
  "NewBranch.tsx": {
    count: 1,
    why: ACTION,
  },
  "NewPersona.tsx": {
    count: 1,
    why: ACTION,
  },
  "NewProject.tsx": {
    count: 2,
    why: ACTION + "; and its progress while it copies the repo, a live value",
  },
  "NewVault.tsx": {
    count: 1,
    why: ACTION,
  },
  "NewWorkspace.tsx": {
    count: 1,
    why: ACTION,
  },
  "NotCloned.tsx": {
    count: 1,
    why: "a clone that failed, said on its own row beside that row's Retry (#1215): inline, per repo",
  },
  "Palette.tsx": {
    count: 8,
    why:
      "the palette's answers, counts and refusals, inside the palette dialog; and " +
      READING +
      ": Finding files…",
  },
  "PanelList.tsx": {
    count: 1,
    why: "a refusal said about one row (a Land's member, #474), inside that row: an inline error, not a line",
  },
  "Panels.tsx": { count: 2, why: "a panel's read refusal and its blocks' tone, inside the panel" },
  "PersonaProfile.tsx": {
    count: 3,
    why: ACTION + ". Also its read refusal: " + READ_ONCE + ". And " + READING,
  },
  "PersonaProfileMissing.tsx": {
    count: 1,
    why: "Create's refusal, inside the form its Notice's fix opened: " + ACTION,
  },
  "ProfileApproval.tsx": {
    count: 1,
    why: WARNING,
  },
  "QuitWarning.tsx": {
    count: 1,
    why: WARNING,
  },
  "RemoveFromWorkspace.tsx": {
    count: 1,
    why: ACTION,
  },
  "RemovePersona.tsx": {
    count: 1,
    why: ACTION,
  },
  "RenameWorkspace.tsx": {
    count: 1,
    why: ACTION,
  },
  "RestoringPane.tsx": {
    count: 1,
    why:
      "A chat's pane before its program has started again (`restoring.rs`): what is happening, " +
      "in the place its terminal takes, as an empty state says it. It asks nothing and offers " +
      "nothing, and it goes when the chat arrives.",
  },
  "RepoInstructionsTab.tsx": {
    count: 2,
    why:
      ACTION +
      '. Also its "Added N files to memory": what the last press answered, not a standing line',
  },
  "RepoPicker.tsx": {
    count: 2,
    why:
      "the repo picker's refusals, inside the dialog that holds it. NO-8: a forge whose CLI " +
      "is not logged in offers its login, typed in a shell tab; the rest are the forge's sentence",
  },
  "SandboxOffer.tsx": { count: 1, why: "the answer's refusal, inside the sandbox offer's Notice" },
  "SavingView.tsx": {
    count: 3,
    why:
      ACTION +
      ". Also a save's own output, line by line: what the last press answered, not a standing line",
  },
  "ReadOnlyTerminal.tsx": {
    count: 1,
    why: "a copy the clipboard refused, said beside the read-only screen it was asked of (#3676)",
  },
  "SearchTab.tsx": { count: 1, why: "the search's progress, a live value" },
  "SkillChangeDialogs.tsx": {
    count: 2,
    why:
      "Rename, Move and Delete's checks and the core's refusals (#3685), beside the fields the " +
      "person is still filling, in the dialog that stays open",
  },
  "SkillCopy.tsx": {
    count: 3,
    why:
      "Make my own copy's refusals (#3686), beside the fields the person is still filling: a " +
      "built-in skill's name and the built-in's description word for word (#3777), each said " +
      "as it is typed, and the core's sentence for the copy just asked for. Make the copy is " +
      "the retry",
  },
  "SkillSources.tsx": {
    count: 4,
    why: "the Sources pages' reads in place (#3687): the line said after a second while rows are held, a read that failed with Try again, the updates still being read, and Install all's review that is more than one read, with its first part to review in place (#4024)",
  },
  "SkillStudioDeleted.tsx": {
    count: 1,
    why:
      "Bring back's refusal of a built-in skill's name for a deleted skill whose own a built-in " +
      "took (#3729), said as it is typed beside the New name field, in the core's sentence. " +
      "Bring back is the retry",
  },
  "SkillStudioTab.tsx": {
    count: 1,
    why: "the Library's reading line (#3681): rows held in place while the skills are read, said after a second",
  },
  "SessionPane.tsx": {
    count: 1,
    why:
      "a runner's chat reconnecting while the runner's link is down (#2496): one quiet state " +
      "over its last screen, as ruled 2026-10-17, never a Notice. It asks nothing and offers " +
      "nothing: purlis links again on its own, and it empties once the chat is reconnected. " +
      "It stays mounted, so its word is heard as it comes.",
  },
  "StartChat.tsx": {
    count: 3,
    why:
      "a dialog's refusal: an inline error, not a Notice (V91n). NO-8: the local file git " +
      "would carry offers the doctor's local-ignore fix, refused profiles link to Settings › " +
      "Harness, a sandbox it cannot apply offers its install, and a start's refusal is ACTION's",
  },
  "TabRename.tsx": { count: 1, why: "a rename's refusal, beside the box being typed in" },
  "ThemeDraftStrip.tsx": {
    count: 4,
    why:
      ACTION +
      ". Four: the draft strip's line that a save is not drawn and which hard floor it " +
      "breaks (#2332), that a refused kept theme's Notice waits while the draft is drawn " +
      "(#2708), what Keep or Revert answered, and Keep's question's refusal",
  },
  "TaskEnd.tsx": {
    count: 2,
    why: ACTION + ". Twice: in the one modal question, and in the second step asked in place",
  },
  "TodoTab.tsx": {
    count: 1,
    why: HEALS,
  },
  "Updates.tsx": {
    count: 4,
    why:
      ACTION +
      ". Also its warning that installing ends every chat (WARNING's case), and the channel " +
      "it moved to: what the last press answered, not a standing line",
  },
  "UseSkill.tsx": {
    count: 4,
    why:
      ACTION +
      ", drawn once under the picker or the arguments form, one shown at a time (#2775). And the " +
      "dialog's warning that the chat's set is past its skill budget, read as it opens: about " +
      "the chat the dialog is for, gone with the dialog. And its line that the chat's workspace " +
      "and persona pin past the most it reads together, with a way to each one's studio (#3391): " +
      "about the same chat, read as the dialog opens and gone with it. And its line that the " +
      "chat's router fell back on the built-in router at its start (SK-15b): what that start " +
      "did, read as the dialog opens and gone with it, with nothing to press: the next chat " +
      "started there asks the router again",
  },
  "VaultSignIn.tsx": {
    count: 5,
    why:
      ACTION +
      ". The set-up of a 1Password vault's sign-in (#1527): why the app's accounts could not " +
      "be listed, a sign-in refused as its vaults were listed, what the test just pressed " +
      "answered (passed, or the core's reason), and the refusal of the step just pressed",
  },
  "Vaults.tsx": {
    count: 2,
    why: READING + ": Reading the vaults…; and Open vault's filter that matches no vault (#2910)",
  },
  "ui/dialog/Dialog.tsx": {
    count: 3,
    why:
      "not a live region: a dialog's role, `dialog` or `alertdialog`, picked by the dialog " +
      "family's own expression (#2910)",
  },
  "VaultTab.tsx": {
    count: 7,
    why: VAULT,
  },
  "Views.tsx": {
    // Three: a region drawn inside another tag's attribute is that region's, counted once
    // (#2910). A Land's refusal beside its row is the row's now (`PanelList.tsx`, #2911).
    count: 3,
    why: VIEWS,
  },
  "Welcome.tsx": {
    count: 2,
    why:
      ACTION +
      ": an open refused, beside the doors and the path box that asked it; and " +
      READING +
      ": the recent projects it could not read",
  },
  "YourRepos.tsx": {
    count: 1,
    why:
      READING +
      ": Get a project's repo list while it is asked for, or why it could not be (#1818); each " +
      "forge's own line below it has its Sign in… or Retry",
  },
  "editor/BranchTree.tsx": { count: 1, why: "the branch tree's read refusal, inside the editor" },
  "editor/PieceDiff.tsx": {
    count: 6,
    why:
      "the comparison tab's whole answer when it draws no merge view: reading, the core's " +
      "refusal, or why no line is drawn (#1189). The tab's own Compare again and Open in your " +
      "editor, at its top, are the way out. And the line that says the comparison drawn is " +
      "being read again after the branch moved, which goes when the new one comes",
  },
  "editor/ToYourEditor.tsx": { count: 1, why: "the piece files' read refusal, inside the editor" },
  "keys/ShortcutsView.tsx": {
    count: 1,
    why: "a filter that matches no shortcut says so where the cheat sheet's list was (#2921)",
  },
  "references.tsx": {
    count: 1,
    why: "the reference picker saying it found no chat, inside its menu",
  },
  "settings/pools.tsx": {
    count: 1,
    why:
      ACTION +
      ". The Pools group's read refusing (#2679), in the core's words: the group is read again " +
      "as it comes on screen. A runner whose key changed has a Notice of its own",
  },
  "settings/PoolProfiles.tsx": {
    count: 1,
    why:
      ACTION +
      ". A pool's profiles refusing (#2049), in the core's words: its Ask again is the way on",
  },
  "settings/accounts.tsx": {
    count: 2,
    why:
      ACTION +
      ". The Accounts group's lines (#1814): a sign-out's or a read's refusal, and what was " +
      "just done; its Sign in with a token and Sign out are the way on",
  },
  "settings/profiles.tsx": {
    count: 2,
    why:
      ACTION +
      ". The Profiles group's lines (#2034): the list's read refusal, and what was just done; " +
      "its Add profile, Import again and Remove… are the way on",
  },
  "settings/Collection.tsx": {
    count: 3,
    why:
      "a collection's Undo answer, a refused Remove with its users and a refused Add (ST-3): " +
      "inline, inside Settings, beside the entry or form they are about",
  },
  "settings/RawToml.tsx": {
    count: 2,
    why: "Edit as TOML's own answer and refusal, inside Settings (NO-7, #1232)",
  },
  "settings/SettingsTab.tsx": {
    count: 1,
    why: "Settings' read refusal, inside its tab (NO-7, #1232)",
  },
  "settings/components.tsx": {
    count: 2,
    why: "the settings set's filter count and a row's refusal: inline, per field",
  },
  "settings/themeGallery.tsx": {
    count: 3,
    why:
      ACTION +
      ". Settings › Theme's lines (#2331): the gallery's read refusal, standing in for its " +
      "cards, what Duplicate as my theme just made, and the values an option's pick dropped " +
      "(#2527); the cards, the group's buttons and the options' choices are " +
      "the way on",
  },
  "settings/thisMachine.tsx": {
    count: 1,
    why: "This machine's read refusal, standing in for its list inside Settings (ST-2; NO-7, #1232)",
  },
  "ui/field/Field.tsx": {
    count: 1,
    why: "a field's error line under its box, why its value is refused, in either of the family's boxes (#2905)",
  },
  "ui/table/TableBar.tsx": {
    count: 1,
    why: "how many rows of a table are chosen, a live value said as it changes (#2911)",
  },
};

/**
 * Notices whose only way out is Copy command (V91q's debt): `file: cause`, each with why the
 * window has no fix of its own.
 *
 * - `slow-start` (NO-4): the relaunch without the session bus. The launch is already made, and
 *   the window cannot start charter again in another environment for the operator.
 */
const COPY_ONLY: string[] = ["App.tsx: slow-start"];

const SRC = join(process.cwd(), "src");

function files(dir: string, keep: (name: string) => boolean): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return files(path, keep);
    return keep(entry.name) ? [path] : [];
  });
}

/** Every source file the window is drawn from: not tests, which name the classes to deny them. */
const sources = () =>
  // A test and a story draw what they test, not a line of the window.
  files(SRC, (name) => /\.tsx?$/.test(name) && !/\.(test|stories)\.tsx?$/.test(name)).map(
    (path) => ({
      name: relative(SRC, path),
      text: readFileSync(path, "utf8"),
    }),
  );

/**
 * Every JSX opening tag in `source` whose name `named` takes, as its tag name and its attribute
 * text. Braces are counted, so an arrow's `>` inside `onClick={() => …}` does not end the tag.
 */
function tags(source: string, named: (tag: string) => boolean): { tag: string; attrs: string }[] {
  const found: { tag: string; attrs: string }[] = [];
  for (const start of source.matchAll(/<([A-Za-z][\w.]*)(?=[\s>/])/g)) {
    const tag = start[1];
    if (!named(tag)) continue;
    let depth = 0;
    let at = (start.index ?? 0) + start[0].length;
    for (; at < source.length; at++) {
      const char = source[at];
      if (char === "{") depth++;
      else if (char === "}") depth--;
      else if (char === ">" && depth === 0) break;
    }
    found.push({ tag, attrs: source.slice((start.index ?? 0) + start[0].length, at) });
  }
  return found;
}

const html = (tag: string) => /^[a-z]/.test(tag);
/** An attribute text with what its expressions hold left out (`{…}` stays `{}`): a tag's own
 *  attributes, and not those of an element passed in one of them (a dialog's `summary`). */
function ownAttributes(attrs: string): string {
  let depth = 0;
  let own = "";
  for (const char of attrs) {
    if (char === "{") {
      if (depth === 0) own += char;
      depth++;
    } else if (char === "}") {
      depth--;
      if (depth === 0) own += char;
    } else if (depth === 0) own += char;
  }
  return own;
}

/** A live region: a `status` or `alert` role (or one picked by an expression), or `aria-live`. */
const live = (attrs: string) => {
  const own = ownAttributes(attrs);
  return /\brole=(?:"(?:status|alert)"|\{)/.test(own) || /\baria-live=/.test(own);
};

/** The class names an attribute text's `className` can hold: every string in its value. */
function classNames(attrs: string): string[] {
  const value = /className=(?:"([^"]*)"|\{([\s\S]*)\})/.exec(attrs);
  if (!value) return [];
  const strings =
    value[1] !== undefined
      ? [value[1]]
      : [...value[2].matchAll(/"([^"]*)"|'([^']*)'|`([^`]*)`/g)].map(([, a, b, c]) =>
          (a ?? b ?? c ?? "").replace(/\$\{[^}]*\}/g, " "),
        );
  return strings.flatMap((one) => one.split(/\s+/)).filter(Boolean);
}

/** The classes only `Notice.tsx` may name: its look and its ways out. */
const noticeOnly = (name: string) =>
  name === "dismiss" || name === "offer" || name === "notice" || name.startsWith("notice-");

/**
 * The Notices in `source` whose only remedy is Copy command, by cause: a `copy` with no fix and
 * no link. A Dismiss beside it does not count, because it hides the problem and fixes nothing.
 */
function copyOnly(source: string): string[] {
  return tags(source, (tag) => tag === "Notice")
    .filter(({ attrs }) => /\bcopy=/.test(attrs) && !/\b(?:fixes|link)=/.test(attrs))
    .map(
      ({ attrs }) =>
        /cause=(?:"([^"]*)"|\{`([^`]*)`\}|\{([^}]*)\})/.exec(attrs)?.slice(1).find(Boolean) ?? "?",
    );
}

/** The Notices in `source` built with a spread (`{...props}`), by cause. */
function spreadNotices(source: string): string[] {
  return tags(source, (tag) => tag === "Notice")
    .filter(({ attrs }) => /\{\s*\.\.\./.test(attrs))
    .map(
      ({ attrs }) =>
        /cause=(?:"([^"]*)"|\{`([^`]*)`\}|\{([^}]*)\})/.exec(attrs)?.slice(1).find(Boolean) ?? "?",
    );
}

describe("the window's standing lines", () => {
  it("are all Notices, but for the live regions listed with why", () => {
    const counted: Record<string, number> = {};
    for (const { name, text } of sources()) {
      if (name === "Notice.tsx") continue;
      // A card's message drawn `live` is a live region too (#3344): only Notice may draw one.
      const n = tags(text, (tag) => tag !== "Notice").filter(
        ({ tag, attrs }) =>
          live(attrs) || (tag === "NoticeMessage" && /\blive\b/.test(ownAttributes(attrs))),
      ).length;
      if (n > 0) counted[name] = n;
    }
    const listed = Object.fromEntries(
      Object.entries(NOT_NOTICES).map(([name, { count }]) => [name, count]),
    );
    expect(counted).toEqual(listed);
  });

  it("have their ways out drawn by Notice alone", () => {
    const named = sources()
      .filter(({ name }) => name !== "Notice.tsx")
      .flatMap(({ name, text }) =>
        tags(text, html)
          .flatMap(({ attrs }) => classNames(attrs))
          .filter(noticeOnly)
          .map((one) => `${name}: ${one}`),
      );
    expect(named).toEqual([]);

    const rules = files(SRC, (name) => name.endsWith(".css")).flatMap((path) =>
      [
        ...readFileSync(path, "utf8")
          .replace(/\/\*[\s\S]*?\*\//g, "")
          .matchAll(/([^{}]+)\{/g),
      ]
        .map(([, selectors]) => selectors.trim())
        .filter((selectors) => /\.(?:dismiss|offer)\b/.test(selectors))
        .map((selectors) => `${relative(SRC, path)}: ${selectors}`),
    );
    expect(rules).toEqual([]);
  });

  it("name their ways out on the Notice, never spread into it", () => {
    // The debt list reads a Notice's own attributes, so one whose ways come in a spread could
    // be copy-only and never be listed (NO-6).
    const spread = sources().flatMap(({ name, text }) =>
      spreadNotices(text).map((cause) => `${name}: ${cause}`),
    );
    expect(spread).toEqual([]);
  });

  it("whose only way out is Copy command are listed as debt", () => {
    const debt = sources().flatMap(({ name, text }) =>
      copyOnly(text).map((cause) => `${name}: ${cause}`),
    );
    expect(debt).toEqual(COPY_ONLY);
  });

  it("would be caught if one came back", () => {
    // The readers above are what the guard rests on, so they are held to what a screen writes.
    const lines = (source: string) =>
      tags(source, (tag) => tag !== "Notice").filter(({ attrs }) => live(attrs));
    expect(lines(`<p className="came-back" role="status">Gone.</p>`)).toHaveLength(1);
    // The reviewer's injection into SessionBusNotice.tsx: any class, a live role.
    expect(lines(`<p className="warning" role="status">The bus is gone.</p>`)).toHaveLength(1);
    expect(lines(`<p className={bad ? "trouble" : "came-back"} role={r}>x</p>`)).toHaveLength(1);
    expect(lines(`<span aria-live="polite">3 found</span>`)).toHaveLength(1);
    expect(
      lines(`<AlertDialog.Description role="alert">No.</AlertDialog.Description>`),
    ).toHaveLength(1);
    // A muted sentence with no live role is not a standing line, and a Notice is the way to draw one.
    expect(lines(`<p className="came-back">Prose.</p>`)).toHaveLength(0);
    expect(lines(`<Notice cause="x" onDismiss={hide}>Gone.</Notice>`)).toHaveLength(0);
    const arrow = `<button className="dismiss" onClick={() => go(a > b)}>Dismiss</button>`;
    expect(tags(arrow, html).flatMap(({ attrs }) => classNames(attrs))).toEqual(["dismiss"]);
    expect(
      copyOnly(`<Notice cause="slow" copy="env X=1 charter" onDismiss={() => hide()}>x</Notice>`),
    ).toEqual(["slow"]);
    expect(
      copyOnly(`<Notice cause="slow" copy="env X=1 charter" fixes={[retry]}>x</Notice>`),
    ).toEqual([]);
    expect(copyOnly("<Notice cause={`gone:${name}`} copy={command}>x</Notice>")).toEqual([
      "gone:${name}",
    ]);
    expect(spreadNotices(`<Notice {...ways} cause="alert:x" at="inbox">x</Notice>`)).toEqual([
      "alert:x",
    ]);
    expect(spreadNotices(`<Notice cause="x" link={go}>x</Notice>`)).toEqual([]);
  });
});
