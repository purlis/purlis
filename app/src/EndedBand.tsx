// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ReactNode } from "react";
import type { Offer } from "./actions";
import { Notice } from "./Notice";
import type { State } from "./chatState";
import { shownState } from "./shownState";
import type { Crumbs } from "./tabChats";

/** Whether a chat's state is its program's end: content (`done`) or not (`failed`). */
export function endedBy(state: State): boolean {
  return state === "done" || state === "failed";
}

/**
 * **How a task that ended says how it ended**, from what its row last said of it (#1484's one
 * function, with its program over): `done`, `failed`, `cancelled` or `ended without a report`.
 */
export function endedState(crumbs: Crumbs) {
  const shown = crumbs.path[crumbs.path.length - 1];
  return shownState({
    board: "done",
    needsYou: false,
    task: { report: shown.report ?? "owed", outcome: shown.outcome, asking: null },
    harness: shown.harness,
  });
}

/** A chat's Resume, where the catalogue offers its Restart row: that row, pressed. */
export function resumeOf(
  offer: Offer | undefined,
  press: (offer: Offer) => void,
): (() => void) | undefined {
  return offer?.available === true ? () => press(offer) : undefined;
}

/**
 * **A chat whose program ended keeps its pane** (#4756, spec #4750 story 21): its last screen
 * stays, read-only, under this band, which offers Resume and Clear pane. The chat's row stays in
 * the sidebar.
 *
 * - **Resume** is the chat's way back: for a chat, the catalogue's Restart row (`chat.restart:<n>`,
 *   a new run on its conversation), drawn only where the catalogue offers it, since a shell has
 *   no conversation; for a task, its finished row's Reopen (#1485), where that row says it can be
 *   reopened.
 * - **Clear pane** shows nothing in the pane in its place, and ends nothing: the chat is still
 *   in the sidebar, where End… takes it away.
 *
 * **A task a pane was left on says how it ended** (#4757, folded here from its own ended view):
 * in its finished row's words where it has one (`how`, and the core's word beside it, `more`),
 * and its report, as text, where the window holds it.
 *
 * A `Notice` at the pane, so it is the one way a standing line is drawn and a polite `status`:
 * a reader hears it once as it comes, and **focus stays where it was**. Nothing here moves it.
 */
export function EndedBand({
  session,
  name,
  how,
  more,
  report,
  resume,
  onClear,
}: {
  session: number;
  /** The chat's name, as its row says it. */
  name: string;
  /** How a task ended, drawn as its finished row draws it (`StateShown`). */
  how?: ReactNode;
  /** The core's own word beside it, where the finished row draws one. */
  more?: string;
  /** A task's report, where the window holds it. Text, never markup. */
  report?: string;
  /** Resume, where the chat has a way back ({@link resumeOf}, or a task's Reopen). */
  resume: (() => void) | undefined;
  onClear: () => void;
}) {
  const clear = { label: "Clear pane", onPress: onClear };
  return (
    <Notice
      cause={`chat-ended:${session}`}
      at="pane"
      label="Ended"
      fixes={resume !== undefined ? [{ label: "Resume", onPress: resume }, clear] : [clear]}
    >
      {name} ended
      {how === undefined ? "." : <>: {how}</>}
      {more !== undefined && <span className="outcome"> {more}</span>}
      {/* What a reader hears as it comes, `steward 4 ended. Resume or Clear pane.` (spec #4750
          story 93): the ways out, which the eye sees as buttons. */}
      <span className="sr-only">
        {" "}
        {resume !== undefined ? "Resume or Clear pane." : "Clear pane."}
      </span>
      {report !== undefined && (
        <section aria-label={`Report from ${name}`}>
          <pre className="block-report-draft ended-report">{report}</pre>
        </section>
      )}
    </Notice>
  );
}
