// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useRef, useState } from "react";
import { commands, type PlaneId, type PlaneSaving } from "./bindings";
import { Notice, type NoticeAction } from "./Notice";
import { askWayOut, FIX_STARTING_SIZE, tellFixStarted, tellSaved } from "./saving";

/** What the conflict Notice is busy with, while a press runs. */
type Busy = "resolve" | "chat" | "continue" | "skip" | "abort";

/**
 * **A save's conflict, resolved from the window** (qw382; ADR 0051, noted 2026-10-14).
 *
 * - On a save whose replay onto the remote conflicted, *Resolve* has the window run the replay
 *   itself and stop where git stops. The terminal stays a way out, with its commands said.
 * - While that replay is stopped, the Notice names the files and offers *Resolve in a chat*
 *   (a chat that edits the files and runs no git), *Continue* (the window checks no conflict
 *   marker is left, stages exactly those files and carries on) and *Abort* (back where it was).
 *   A commit the settling left empty offers *Skip*, which drops it.
 * - When the replay finishes, what is committed is pushed and nothing new is committed: a file
 *   changed meanwhile is named and left for the person.
 *
 * Every git command is the window's own press: a chat may not rewrite the project root's tree,
 * and nothing it writes decides which files are staged or what runs.
 */
export function SaveConflict({
  plane,
  saving,
  onSaved,
}: {
  plane: PlaneId;
  saving: PlaneSaving;
  onSaved?: () => void;
}) {
  const [busy, setBusy] = useState<Busy>();
  const [said, setSaid] = useState<string>();
  /** The last Continue found the commit being replayed has nothing left to commit: Skip is
   *  offered, which drops it. */
  const [empty, setEmpty] = useState(false);
  /** The files git stopped in, as the last answer named them: Continue stages them with what
   *  git still calls unmerged, so a file settled and staged by hand is still checked. */
  const shown = useRef<string[]>([]);
  const sending = useRef(false);

  const step = (which: "start" | "continue" | "skip" | "abort") => {
    if (sending.current) return;
    sending.current = true;
    setBusy(which === "start" ? "resolve" : which);
    setSaid(undefined);
    const files = [...new Set([...shown.current, ...saving.conflicts])];
    void commands
      .settleSaveConflict(plane, which, files)
      .then((answer) => {
        if (answer.status === "error") {
          setSaid(answer.error);
          return;
        }
        const got = answer.data;
        setEmpty(got.outcome === "empty");
        if (got.outcome === "stopped") shown.current = got.files;
        if (got.outcome === "saved" || got.outcome === "aborted") shown.current = [];
        setSaid(got.said);
        if (got.outcome === "saved") onSaved?.();
      })
      .catch((err: unknown) => setSaid(String(err)))
      .finally(() => {
        sending.current = false;
        setBusy(undefined);
        tellSaved();
      });
  };

  const inAChat = () => {
    if (sending.current) return;
    sending.current = true;
    setBusy("chat");
    setSaid(undefined);
    void commands
      .resolveSaveInAChat(plane, FIX_STARTING_SIZE.columns, FIX_STARTING_SIZE.rows)
      .then((answer) => {
        if (answer.status === "ok") tellFixStarted(plane, answer.data);
        else setSaid(answer.error);
      })
      .catch((err: unknown) => setSaid(String(err)))
      .finally(() => {
        sending.current = false;
        setBusy(undefined);
      });
  };

  const files = saving.conflicts.length > 0 && (
    <ul className="saving-files" aria-label="Where it conflicts">
      {saving.conflicts.map((path) => (
        <li key={path}>{path}</li>
      ))}
    </ul>
  );
  const outcome = said !== undefined && <span className="alert-said">{said}</span>;
  const terminal: NoticeAction = {
    label: "Open terminal here",
    onPress: () => askWayOut(plane, "terminal"),
  };

  if (saving.stoppedBy === "rebase") {
    const chat: NoticeAction = { label: "Resolve in a chat", onPress: inAChat, busy: !!busy };
    const carryOn: NoticeAction = {
      label: "Continue",
      onPress: () => step("continue"),
      busy: !!busy,
    };
    const abort: NoticeAction = { label: "Abort", onPress: () => step("abort"), busy: !!busy };
    const skip: NoticeAction = { label: "Skip", onPress: () => step("skip"), busy: !!busy };
    return (
      <Notice
        cause="saving-resolving"
        tone="trouble"
        fixes={
          empty
            ? [skip, abort]
            : saving.conflicts.length > 0
              ? [chat, carryOn, abort]
              : [carryOn, abort]
        }
        link={terminal}
        under={files || undefined}
      >
        {saving.conflicts.length > 0
          ? "The replay is stopped on a conflict. Settle the files below, in a chat or by hand, then Continue. Abort puts the project back where it was."
          : "The replay is stopped, with nothing left in conflict. Continue carries it on, and Abort puts the project back where it was."}
        {outcome}
      </Notice>
    );
  }
  if (saving.byHand == null) return null;
  const resolve: NoticeAction = {
    label: "Resolve",
    onPress: () => step("start"),
    busy: busy === "resolve",
  };
  return (
    <Notice
      cause="saving-rebase-by-hand"
      tone="trouble"
      fixes={[resolve]}
      link={terminal}
      under={files || undefined}
    >
      {`Resolve replays this project's own commits onto the remote's and stops where they conflict, for you or a chat to settle. ${saving.byHand}`}
      {outcome}
    </Notice>
  );
}
