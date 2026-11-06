// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { emit } from "@tauri-apps/api/event";
import { dismissMovedAfterSignIn, forgetMove } from "./chatMoves";
import { hideForNow } from "./dismissals";
import { thisWindow } from "./here";
import { forgetInputRefused } from "./inputRefused";
import { putAwayDraftsReady } from "./skillDraftAsks";
import { NOTE_CLEARED, type KeptOutside, type NoteCleared } from "./tabWindows";

/**
 * **A note put away in one window is put away in every window of the project** (#5225, #5363).
 * Each window hears what its chats say from when it is listening, so another may hold the same
 * note for the chat coming back to it. Whatever holds the note, PlaneView's state or a store of
 * the window's own ({@link KeptOutside}), says so here as it puts it away.
 */

/** Tells every window of `plane`'s project, this one too, that chat `session`'s `note` was put
 *  away here. */
export function clearedHere(plane: string, session: number, note: NoteCleared["note"]): void {
  void emit(NOTE_CLEARED, {
    plane,
    session,
    note,
    from: thisWindow(),
  } satisfies NoteCleared).catch(() => undefined);
}

/** Lets go here of chat `session`'s note that another window of `plane`'s project put away,
 *  where this window keeps it in a store of its own. The core is told nothing: that window was. */
export function letGoHere(plane: string, session: number, note: KeptOutside): void {
  switch (note.kind) {
    case "input-refused":
      forgetInputRefused(plane, session);
      return;
    case "move":
      forgetMove(plane, session);
      return;
    case "moved":
      dismissMovedAfterSignIn(plane, session);
      return;
    case "hidden":
      for (const cause of note.causes) hideForNow(plane, cause);
      return;
    case "draft-ready":
      putAwayDraftsReady(plane, session, note.drafts);
      return;
  }
}

/** Whether `note` is one a store of the window's own keeps, which {@link letGoHere} lets go. */
export function keptOutside(note: NoteCleared["note"]): note is KeptOutside {
  return (
    note.kind === "input-refused" ||
    note.kind === "move" ||
    note.kind === "moved" ||
    note.kind === "hidden" ||
    note.kind === "draft-ready"
  );
}
