// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useContext, useState } from "react";
import type { PlaneId, Shown } from "./bindings";
import { answerTaken, asksMoved } from "./asks";
import { AskWrites } from "./AskWrites";
import { Notice, NoticeOf, type NoticeAction } from "./Notice";

/**
 * **A change this chat proposed to purlis's own configuration, on its tab** (#1975). A chat
 * may not write the project's manifest, a persona's `mcp.json` or `persona.md`, or a skill's
 * files: its edit of one is held here for you instead, the file as purlis reads it and as the
 * change would make it, and
 *
 * - **Apply**: writes exactly that, as you, only while the file is still what the chat read
 *   (it rides your next save); offered only where the change is shown whole, and only once it
 *   is on the screen.
 * - **Dismiss**: writes nothing.
 *
 * There is no "always apply", and Apply allows nothing: a tool or host the change declares
 * still asks its own Allow. The chat is told applied, dismissed or stale on its next turn.
 *
 * Drawn from the asks registry (`asks`), so an answer in the Inbox clears it here, and its
 * answers carry the registry's labels.
 */
export function ProposedChangeNotice({
  plane,
  session,
  asks,
  onWay,
}: {
  plane: PlaneId;
  session: number;
  /** This chat's asks its pane draws, of which the proposals are taken. */
  asks: readonly Shown[];
  /** An answer was pressed: its words, and the ask it answered. */
  onWay?: (words: string, ask: Shown) => void;
}) {
  /** "this chat" on its own pane; "it" where its path is said first (#1538). */
  const offScreen = useContext(NoticeOf) !== null;
  const [refused, setRefused] = useState<Readonly<Record<string, string>>>({});
  const [busy, setBusy] = useState<string>();
  /** The asks whose change is on the screen: Apply counts only beside one. */
  const [drawn, setDrawn] = useState<ReadonlySet<string>>(new Set());
  return (
    <>
      {asks.map((ask) => {
        const path = ask.answer;
        if (ask.session !== session || path.via !== "proposed-change") return null;
        const shownWhole = ask.writes?.kind === "whole";
        const [first, ...rest] = ask.options.map((option): NoticeAction => ({
          label: option.label,
          // Apply is a grant, drawn as every grant is (ADR 0097 §6).
          allows: option.allows,
          busy: busy === ask.ask || (option.allows && shownWhole && !drawn.has(ask.ask)),
          onPress: () => {
            if (busy !== undefined) return;
            if (option.allows && !drawn.has(ask.ask)) return;
            setBusy(ask.ask);
            setRefused((was) =>
              Object.fromEntries(Object.entries(was).filter(([key]) => key !== ask.ask)),
            );
            onWay?.(option.label, ask);
            void answerTaken(plane, ask, option.id)
              .then((answer) => {
                const why = answer.refused;
                if (why !== undefined) setRefused((was) => ({ ...was, [ask.ask]: why }));
              })
              .finally(() => {
                setBusy(undefined);
                asksMoved(plane);
              });
          },
        }));
        if (first === undefined) return null;
        const said = refused[ask.ask];
        const under =
          ask.writes === undefined || ask.writes === null ? undefined : (
            <AskWrites
              writes={ask.writes}
              onDrawn={() =>
                setDrawn((was) => (was.has(ask.ask) ? was : new Set(was).add(ask.ask)))
              }
            >
              {null}
            </AskWrites>
          );
        return (
          <Notice
            key={ask.ask}
            cause={`proposed-change:${ask.ask}`}
            at="pane"
            tone={said === undefined ? "news" : "trouble"}
            label="Proposed change"
            fixes={[first, ...rest]}
            under={under}
          >
            {ask.says}. {offScreen ? "It" : "This chat"} may not write this file itself, so only you
            apply it, here or in the Inbox; {offScreen ? "it" : "this chat"} is told what you did on
            its next turn.
            {said !== undefined && ` ${said}`}
          </Notice>
        );
      })}
    </>
  );
}
