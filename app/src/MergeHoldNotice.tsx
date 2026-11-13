// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import type { PlaneId, Shown } from "./bindings";
import { answerTaken, asksMoved } from "./asks";
import { Notice, type NoticeAction } from "./Notice";
import { MergeFacts } from "./MergeFacts";
import { confirmationOf, rereadMerge } from "./mergeHold";
import { NOTICE_MOVED_JUST_NOW, SETTLE_MS } from "./noticeSettle";

/**
 * **A held merge on its chat's pane** (T1-39; R-114, Q239): the merge its chat's hook held,
 * where the person looks at the chat, as well as on its row in Asks, from the asks registry, so
 * an answer in either place clears both.
 *
 * - **Merge** is the app's own forge call, at the head the ask was raised at, and asks once more
 *   first, in the risk's own sentence (`Shown.risk.confirm`), or purlis's own where the hold
 *   arrived without its risk (Q268): Merge again acts, Cancel takes the
 *   question back. A confirmation holds only for the ask and the question it was asked of, so a
 *   hold raised again at a moved head (Q242), or whose facts refreshed (Q248), asks from the
 *   start. **Merge reads the request again before it asks once more** (Q248, T1-40), so the
 *   question is asked of what the forge says now; and **the Merge it asks with waits to settle**,
 *   so a double click that opened the question doesn't answer it too.
 * - **Don't merge** merges nothing.
 *
 * It shows what the merge row does (`MergeFacts`, T1-40): the checks as of the forge's read, the
 * ruleset state with the gap beside it, and the title last, as text the chat may have written.
 * A hold is never a rule (R-143): nothing here is remembered, whatever the ask carries. Every
 * word but the ask's own sentences and the forge's facts is purlis's.
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
  const question = confirmationOf(hold);
  const confirming = asked === question;
  /** The question asked once more has been in sight long enough for its Merge to act. */
  const [settledFor, setSettledFor] = useState<string>();
  const settled = confirming && settledFor === question;
  useEffect(() => {
    if (!confirming) return;
    const after = setTimeout(() => setSettledFor(question), SETTLE_MS);
    return () => clearTimeout(after);
  }, [confirming, question]);

  /** Merge's first press: the request read again, and the question asked of what it says. */
  const askAgain = () => {
    if (busy) return;
    setBusy(true);
    setRefused(undefined);
    void rereadMerge(plane, hold)
      .catch((err: unknown) => ({ refused: String(err), question: undefined }))
      .then((read) => {
        if (read.refused !== undefined) setRefused(read.refused);
        else if (read.question !== undefined) {
          // A question asked anew settles anew, though it was asked and taken back before.
          setSettledFor(undefined);
          setAsked(read.question);
        }
      })
      .finally(() => setBusy(false));
  };

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
    // Merge asks once more before it acts (Q239, Q268), and the Merge it asks with acts only
    // once the question has settled.
    onPress: () => {
      if (!confirming) askAgain();
      else if (settled) send(merge.id, merge.label);
      else setRefused(NOTICE_MOVED_JUST_NOW);
    },
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
      {refused !== undefined && ` ${refused}`}{" "}
      {hold.answer.via === "merge-hold" && <MergeFacts facts={hold.answer.facts} />}
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
