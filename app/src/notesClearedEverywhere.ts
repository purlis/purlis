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

/** **How each note kept outside PlaneView is let go here**, by its kind: one list, which both
 *  {@link letGoHere} and {@link keptOutside} read. */
const LET_GO: {
  [K in KeptOutside["kind"]]: (
    plane: string,
    session: number,
    note: Extract<KeptOutside, { kind: K }>,
  ) => void;
} = {
  "input-refused": (plane, session) => forgetInputRefused(plane, session),
  move: (plane, session) => forgetMove(plane, session),
  moved: (plane, session) => dismissMovedAfterSignIn(plane, session),
  hidden: (plane, _session, note) => {
    for (const cause of note.causes) hideForNow(plane, cause);
  },
  "draft-ready": (plane, session, note) => putAwayDraftsReady(plane, session, note.drafts),
};

/** Lets go here of chat `session`'s note that another window of `plane`'s project put away,
 *  where this window keeps it in a store of its own. The core is told nothing: that window was. */
export function letGoHere(plane: string, session: number, note: KeptOutside): void {
  (LET_GO[note.kind] as (plane: string, session: number, note: KeptOutside) => void)(
    plane,
    session,
    note,
  );
}

/** **Puts chat `session`'s `note` away here and in every window of the project**: what a Notice
 *  whose note a store of the window's own keeps does on its Dismiss. */
export function putAwayEverywhere(plane: string, session: number, note: KeptOutside): void {
  letGoHere(plane, session, note);
  clearedHere(plane, session, note);
}

/** Whether `note` is one a store of the window's own keeps, which {@link letGoHere} lets go. */
export function keptOutside(note: NoteCleared["note"]): note is KeptOutside {
  return note.kind in LET_GO;
}
