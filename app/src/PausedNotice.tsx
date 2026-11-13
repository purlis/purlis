// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import type { PlaneId, Shown } from "./bindings";
import { RAISE_THE_BUDGET, answerTaken, asksMoved } from "./asks";
import { Notice, type NoticeAction } from "./Notice";

/**
 * **A chat paused at its budget, or as looping, on its tab** (#6015, ADR 0121 decision 20). The
 * chat is held still until the person answers, here or on its row in Asks:
 *
 * - **Budget reached**: Continue, Raise the budget (which opens Settings › Project › Dispatch at
 *   its limits; the chat goes on once it is within the new limit), or Stop.
 * - **Looping**: It's fine, continue, or Stop.
 * - **Blocked on the same thing again and again** (T1-36): Tell it to stop trying, Let it keep
 *   trying, or Stop chat.
 *
 * Drawn from the asks registry (`asks`), so an answer in Asks clears it here. Every word is
 * purlis's: what it was paused for is the core's sentence, and the answers are its labels.
 */
export function PausedNotice({
  plane,
  session,
  asks,
  onWay,
}: {
  plane: PlaneId;
  session: number;
  /** This chat's asks its pane draws, of which its pause is taken. */
  asks: readonly Shown[];
  /** An answer was pressed: its words, and the ask it answered. */
  onWay?: (words: string, ask: Shown) => void;
}) {
  const [refused, setRefused] = useState<string>();
  const [busy, setBusy] = useState(false);
  const ask = asks.find((one) => one.session === session && one.answer.via === "paused");
  if (ask === undefined) return null;
  const [first, ...rest] = ask.options.map((option): NoticeAction => ({
    label: option.label,
    busy,
    onPress: () => {
      if (busy) return;
      setBusy(true);
      setRefused(undefined);
      // Raise the budget opens Settings and answers nothing: the chat stays paused meanwhile.
      if (option.id !== RAISE_THE_BUDGET) onWay?.(option.label, ask);
      void answerTaken(plane, ask, option.id)
        .then((answer) => setRefused(answer.refused))
        .finally(() => {
          setBusy(false);
          asksMoved(plane);
        });
    },
  }));
  if (first === undefined) return null;
  return (
    <Notice
      cause={`paused:${session}`}
      at="pane"
      tone={refused === undefined ? "news" : "trouble"}
      label={ask.source === "budget" ? "Budget reached" : "Paused"}
      since={ask.since}
      waits
      fixes={[first, ...rest]}
    >
      {ask.says} It is paused until you answer.
      {refused !== undefined && ` ${refused}`}
    </Notice>
  );
}
