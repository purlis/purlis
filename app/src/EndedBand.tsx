// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Offer } from "./actions";
import { Notice } from "./Notice";
import { stateOf, useChatsHere, useChatsSelect, type State } from "./chatState";

/** Whether a chat's state is its program's end: content (`done`) or not (`failed`). */
export function endedBy(state: State): boolean {
  return state === "done" || state === "failed";
}

/** Whether chat `session`'s program has ended, as the core last told it (`chat-moved`). */
export function useChatEnded(session: number): boolean {
  return useChatsSelect(useChatsHere(), (states) => endedBy(stateOf(states, session)));
}

/**
 * **A chat whose program ended keeps its pane** (#4756, spec #4750 story 21): its last screen
 * stays, read-only, under this band, which offers Resume and Clear pane. The chat's row stays in
 * the sidebar.
 *
 * - **Resume** is the catalogue's Restart row for the chat (`chat.restart:<n>`, a new run on its
 *   conversation), drawn only where the catalogue offers it: a shell has no conversation.
 * - **Clear pane** shows nothing in the pane in its place, and ends nothing: the chat is still
 *   in the sidebar, where End… takes it away.
 *
 * A `Notice` at the pane, so it is the one way a standing line is drawn and a polite `status`:
 * a reader hears it once as it comes, and **focus stays where it was**. Nothing here moves it.
 */
export function EndedBand({
  session,
  name,
  resume,
  onResume,
  onClear,
}: {
  session: number;
  /** The chat's name, as its row says it. */
  name: string;
  /** The catalogue's Restart row for the chat, where it has one. */
  resume: Offer | undefined;
  onResume: (offer: Offer) => void;
  onClear: () => void;
}) {
  const resumable = resume?.available === true;
  const clear = { label: "Clear pane", onPress: onClear };
  return (
    <Notice
      cause={`chat-ended:${session}`}
      at="pane"
      label="Ended"
      fixes={resumable ? [{ label: "Resume", onPress: () => onResume(resume) }, clear] : [clear]}
    >
      {name} ended.
      {/* What a reader hears as it comes, `steward 4 ended. Resume or Clear pane.` (spec #4750
          story 93): the ways out, which the eye sees as buttons. */}
      <span className="sr-only"> {resumable ? "Resume or Clear pane." : "Clear pane."}</span>
    </Notice>
  );
}
