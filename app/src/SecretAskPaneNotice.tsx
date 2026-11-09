// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useId, useState } from "react";
import type { Offered, PlaneId, Shown } from "./bindings";
import { answerTaken, asksMoved, secretAskEntries } from "./asks";
import { Notice } from "./Notice";
import { SecretAskNotice, type SecretAskGiven } from "./SecretAskNotice";

/** The label each kind's Notice is named, by the path it is answered on. */
const DRAWN: Readonly<Partial<Record<Shown["answer"]["via"], string>>> = {
  // A key a chat's brokered run missed (#5068, ADR 0109).
  secret: "Secret",
  // A run of a program its persona does not declare, or of keys it lacks (ADR 0112, #5242).
  "run-grant": "Run with a secret",
};

/**
 * **One secret's ask, on its chat's pane** (plane #2697): the core's line, and Answer, which
 * opens the secret's one notice under it (`SecretAskNotice`, as the Inbox draws it): its facts,
 * its fields, its Allow and its own answers. Its values are read from their boxes only as an
 * answer is pressed, and sent through the path the ask names, as the Inbox sends them.
 */
function OneSecret({
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
  const formId = useId();
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [said, setSaid] = useState<string>();
  const answer = (option: Offered, given: () => SecretAskGiven) => {
    if (busy) return;
    setBusy(true);
    setSaid(undefined);
    const got = given();
    void answerTaken(
      plane,
      ask,
      option.id,
      undefined,
      got.typed,
      got.picked,
      got.remember,
      got.run,
      got.named,
    )
      .then(({ refused }) => {
        if (refused === undefined) onWay?.(option.label, ask);
        else setSaid(refused);
      })
      .finally(() => {
        setBusy(false);
        asksMoved(plane);
      });
  };
  return (
    <Notice
      cause={`${ask.ask}:${ask.says}`}
      at="pane"
      tone={said === undefined ? "news" : "trouble"}
      label={label}
      since={ask.since}
      waits
      fixes={[
        {
          label: "Answer",
          onPress: () => setOpen((was) => !was),
          opens: { id: formId, open },
        },
      ]}
      under={
        open ? (
          <div id={formId}>
            <SecretAskNotice
              ask={ask}
              chat="This chat"
              entries={() => secretAskEntries(plane, ask)}
              busy={busy}
              onAnswer={answer}
              onSaid={setSaid}
            />
          </div>
        ) : undefined
      }
    >
      {ask.says}
      {said !== undefined && ` ${said}`}
    </Notice>
  );
}

/**
 * **What a chat's runs ask of the person's secrets, on its pane** (plane #2697): a key its
 * brokered run missed, and a run of a program its persona does not declare. The command waits on
 * the answer, so it is drawn where the person looks at the chat, not only in Asks.
 */
export function SecretAskPaneNotice({
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
        return <OneSecret key={ask.ask} plane={plane} ask={ask} label={label} onWay={onWay} />;
      })}
    </>
  );
}
