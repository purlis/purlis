// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import type { PlaneId, Shown } from "./bindings";
import { answerTaken, asksMoved } from "./asks";
import { Notice, type NoticeAction } from "./Notice";

/**
 * **A held merge on its chat's pane** (T1-39; R-114, Q239): the merge its chat's hook held,
 * where the person looks at the chat, as well as on its row in Asks, from the asks registry, so
 * an answer in either place clears both.
 *
 * - **Merge** is the app's own forge call, at the head the ask was raised at, and asks once more
 *   first, in the risk's own sentence (`Shown.risk.confirm`), or purlis's own where the hold
 *   arrived without its risk (Q268): Merge again acts, Cancel takes the
 *   question back. A confirmation holds only for the ask and the question it was asked of, so a
 *   hold raised again at a moved head (Q242) asks from the start.
 * - **Don't merge** merges nothing.
 *
 * A hold is never a rule (R-143): nothing here is remembered, whatever the ask carries. The
 * merge row's own contents (the ruleset, the gap) are T1-40's (R-120). Every word but the ask's
 * own sentences is purlis's.
 */
export function MergeHoldNotice({
  plane,
  session,
  asks,
  onWay,
}: {
  plane: PlaneId;
  session: number;
  /** This chat's asks its pane draws, of which its holds are taken. */
  asks: readonly Shown[];
  /** An answer was pressed: its words, and the ask it answered. */
  onWay?: (words: string, ask: Shown) => void;
}) {
  const holds = asks.filter((one) => one.session === session && one.answer.via === "merge-hold");
  return (
    <>
      {holds.map((hold) => (
        <OneHold key={hold.ask} plane={plane} hold={hold} onWay={onWay} />
      ))}
    </>
  );
}

function OneHold({
  plane,
  hold,
  onWay,
}: {
  plane: PlaneId;
  hold: Shown;
  onWay?: (words: string, ask: Shown) => void;
}) {
  const [refused, setRefused] = useState<string>();
  const [busy, setBusy] = useState(false);
  /** The question Merge asked once more, for the ask it was asked of. */
  const [asked, setAsked] = useState<string>();
  // A merge always asks once more (Q268): in the risk's own sentence, or in purlis's own where
  // the hold arrived without its risk, so it never merges on the first press.
  const confirm = hold.risk?.confirm ?? fallbackConfirm(hold);
  // As the row's (`InboxAsks`): the answer's facts are part of the question, so a refresh of
  // them (Q248) takes an open confirmation back.
  const question = [hold.ask, hold.risk?.confirm ?? "", JSON.stringify(hold.answer)].join("\n");
  const confirming = asked === question;

  const send = (option: string, label: string) => {
    if (busy) return;
    setBusy(true);
    setRefused(undefined);
    onWay?.(label, hold);
    void answerTaken(plane, hold, option)
      .catch((err: unknown) => ({ refused: `purlis could not send it: ${String(err)}` }))
      .then((answer) => setRefused(answer.refused))
      .finally(() => {
        setBusy(false);
        setAsked(undefined);
        asksMoved(plane);
      });
  };
  const offered = (id: string) => hold.options.find((one) => one.id === id);
  const merge = offered("merge");
  const dont = offered("dont-merge");
  if (merge === undefined || dont === undefined) return null;

  const mergeWay: NoticeAction = {
    label: merge.label,
    allows: true,
    busy,
    // Merge asks once more before it acts (Q239, Q268).
    onPress: () => (confirming ? send(merge.id, merge.label) : setAsked(question)),
  };
  const fixes: readonly [NoticeAction, ...NoticeAction[]] = confirming
    ? [mergeWay, { label: "Cancel", busy, onPress: () => setAsked(undefined) }]
    : [mergeWay, { label: dont.label, busy, onPress: () => send(dont.id, dont.label) }];
  return (
    <Notice
      cause={`merge-hold:${hold.ask}`}
      at="pane"
      tone={refused === undefined ? "news" : "trouble"}
      label="Merge held"
      since={hold.since}
      waits
      fixes={fixes}
    >
      {confirming ? confirm : hold.risk == null ? hold.says : `${hold.says} ${hold.risk.why}.`}
      {refused !== undefined && ` ${refused}`}
    </Notice>
  );
}

/** purlis's own question for a hold that arrived without its risk (Q268). */
function fallbackConfirm(hold: Shown): string {
  const path = hold.answer;
  return path.via === "merge-hold"
    ? `Merge request ${path.number} of ${path.repo} at ${path.head.slice(0, 7)}? There is no undo.`
    : "Merge this request? There is no undo.";
}
