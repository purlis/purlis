// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import type { PlaneId, Shown } from "./bindings";
import { answerTaken, asksMoved } from "./asks";
import { Notice, type NoticeAction } from "./Notice";

/** The asks of a chat's browser its pane draws here, by the path each is answered on, and the
 *  label its Notice is named. */
const DRAWN: Readonly<Partial<Record<Shown["answer"]["via"], string>>> = {
  // What a chat's browser may reach, asked at its first browser call (#2013).
  browser: "Browser",
  // Sites new to the project its browser held back (#2018).
  "browser-sites": "Browser site",
  // A page that needs the person, such as a consent page or a second factor (#2015): Take over
  // takes the browser and opens its view.
  "take-over": "Browser needs you",
};

/**
 * **One ask of a chat's, on its pane, answered by its own buttons**: the core's line and the
 * core's answers, each sent through the path the ask names (`answerTaken`), as the Inbox's
 * Answer sends it. A grant among them waits for the Notice to settle, as every Notice that
 * grants does. A chat's browser asks are drawn so, and its asks on a pool's runner
 * (`PoolAskNotice`).
 */
export function OneAsk({
  plane,
  ask,
  label,
  onWay,
}: {
  plane: PlaneId;
  ask: Shown;
  label: string;
  onWay?: (answer: string, ask: Shown) => void;
}) {
  const [refused, setRefused] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [first, ...rest] = ask.options.map((option): NoticeAction => ({
    label: option.label,
    busy,
    allows: option.allows,
    onPress: () => {
      if (busy) return;
      setBusy(true);
      setRefused(undefined);
      void answerTaken(plane, ask, option.id)
        .then((answer) => {
          if (answer.refused === undefined) onWay?.(option.label, ask);
          else setRefused(answer.refused);
        })
        .finally(() => {
          setBusy(false);
          asksMoved(plane);
        });
    },
  }));
  if (first === undefined) return null;
  return (
    <Notice
      cause={`${ask.ask}:${ask.says}`}
      at="pane"
      tone={refused === undefined ? "news" : "trouble"}
      label={label}
      since={ask.since}
      fixes={[first, ...rest]}
    >
      {ask.says}
      {refused !== undefined && ` ${refused}`}
    </Notice>
  );
}

/**
 * **What this chat's browser asks the person, on its pane**: what it may reach, the sites new
 * to the project it held back, and a page that waits for the person to take the browser over. The chat waits on the answer, so it is drawn where the person
 * looks at the chat, not only in Asks.
 */
export function BrowserAskNotice({
  plane,
  session,
  asks,
  onWay,
}: {
  plane: PlaneId;
  session: number;
  asks: readonly Shown[];
  onWay?: (answer: string, ask: Shown) => void;
}) {
  return (
    <>
      {asks.map((ask) => {
        const label = DRAWN[ask.answer.via];
        if (ask.session !== session || label === undefined) return null;
        return <OneAsk key={ask.ask} plane={plane} ask={ask} label={label} onWay={onWay} />;
      })}
    </>
  );
}
