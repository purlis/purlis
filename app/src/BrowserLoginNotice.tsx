// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useContext, useState } from "react";
import type { PlaneId, Shown } from "./bindings";
import { answerTaken, asksMoved } from "./asks";
import { LoginSite, useTypedSite } from "./LoginSite";
import { Notice, NoticeOf, type NoticeAction } from "./Notice";

/**
 * **A browser login a chat suggested, on its pane** (#4595): the sandbox's own Notice and
 * answers, held until it settles as every Notice that grants is. Its call waits for the press,
 * and only a press fills anything.
 */
function OneLogin({
  plane,
  ask,
  onWay,
}: {
  plane: PlaneId;
  ask: Shown;
  onWay?: (answer: string, ask: Shown) => void;
}) {
  const offScreen = useContext(NoticeOf) !== null;
  const [typed, setTyped] = useTypedSite(ask);
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
      void answerTaken(plane, ask, option.id, undefined, typed)
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
  if (first === undefined || ask.login == null) return null;
  return (
    <Notice
      cause={`browser-login:${ask.ask}`}
      at="pane"
      tone={refused === undefined ? "news" : "trouble"}
      label="Browser login"
      fixes={[first, ...rest]}
    >
      {offScreen ? "It asks" : "This chat asks"} to log its browser in here, and waits for your
      answer.
      <LoginSite login={ask.login} typed={typed} onTyped={setTyped} />
      {refused !== undefined && ` ${refused}`}
    </Notice>
  );
}

/** Every login this chat's pane draws. */
export function BrowserLoginNotice({
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
      {asks
        .filter((ask) => ask.session === session && ask.answer.via === "browser-login")
        .map((ask) => (
          <OneLogin key={ask.ask} plane={plane} ask={ask} onWay={onWay} />
        ))}
    </>
  );
}
