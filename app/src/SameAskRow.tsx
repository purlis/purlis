// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useId, useLayoutEffect, useRef, useState } from "react";
import type { Offered, PlaneId, Shown } from "./bindings";
import { answerTaken, asksMoved } from "./asks";
import { AskDecision } from "./AskDecision";
import { AskStanding } from "./AskStanding";
import { sameSaid, type Same } from "./asksSame";
import { ALLOW_OR_KEEP, MOVED_JUST_NOW, SOURCE_SAID } from "./InboxAsks";
import { KindIcon, kindOfAsk, RowAge } from "./InboxRow";
import { ageOf, heldKey, noteAnswered } from "./inboxRules";
import { chainSaid, useSettledPlace } from "./inboxRowParts";
import { toneOfKind } from "./messageTone";
import { askSeenKey, useSeenRow, type SeenRows } from "./seenRows";
import { capMarks } from "./combiningMarks";
import { Button } from "./ui/button/Button";
import {
  MessageBody,
  MessageMeta,
  MessageRow,
  MessageState,
  MessageTop,
} from "./ui/message/Message";
import { Tooltip } from "./ui/tooltip/Tooltip";

/** What the grant that answers every chat of a group says. */
export const ALLOW_FOR_ALL = "Allow for all";

/** What opens a group's chats, each answered on its own row. */
export const ANSWER_EACH = "Answer each";

/** What a group's row says where some of its chats were not allowed. */
export const allowedSome = (allowed: number, of: number, who: string, refused: string) =>
  `Allowed for ${allowed} of ${of} chats. ${who}: ${refused}`;

/** What a group's row says Allow for all does: each chat gets its own narrowest grant. */
export const forAllSaid = (grant: Offered) =>
  `${ALLOW_FOR_ALL} answers each chat with ${grant.label}.`;

/** The key of a group's row, as its places and its section know it. */
export const sameRowKey = (same: Same) => `same:${heldKey(same.placed[0].ask, undefined)}`;

/** **The stops of a group's row, in order** (#6009): the row, Allow for all, Answer each. */
export const samePlaces = (same: Same): string[] => {
  const key = sameRowKey(same);
  return [key, `${key}:${same.grant.id}`, `${key}:each`];
};

/**
 * **Identical requests from several chats, answered once** (ADR 0121 decision 6, #6009): one
 * row, "3 chats want to reach api.example.com", drawn as a decision is (`AskDecision`, #5998).
 * Its one answer, **Allow for all**, answers each chat's ask once with the narrowest grant that
 * ask offers (Allow once, or Allow for this chat), never wider; its settle guard holds it as any
 * Allow in Asks. **Answer each** opens the chats' own rows under it, where each is answered,
 * kept blocked or given a wider scope on its own.
 *
 * **Only the asks listed in the scope in view** (decision 8 as amended 2026-11-12, #6346): the
 * group holds what `withSame` grouped from them, all of this project, so Allow for all never
 * answers an ask the person cannot see.
 */
export function SameAsk({
  plane,
  same,
  open,
  onOpen,
  above,
  now,
  onAnswered,
  seen,
}: {
  plane: PlaneId;
  /** The seen record of its first chat's project, where that is another's (#1921). */
  seen?: SeenRows | null;
  same: Same;
  /** Its chats' own rows are drawn under it. */
  open: boolean;
  onOpen: (open: boolean) => void;
  /** The stops drawn above it: one changing moves it (`useSettledPlace`). */
  above: string;
  now: number;
  onAnswered?: (ask: Shown, option: Offered, live: boolean) => void;
}) {
  const id = useId();
  const [said, setSaid] = useState<string>();
  const [busy, setBusy] = useState(false);
  const first = same.placed[0].ask;
  const asks = same.placed.map(({ ask }) => ask);
  const title = sameSaid(same);
  const who = asks.map((ask) => chainSaid(ask.chain)).join(", ");
  const line = capMarks(first.says).text;
  const key = sameRowKey(same);
  /** Its watch to be seen, once its row is attached (`useSettledPlace`). */
  const seenAttached = useRef<() => void>(undefined);
  const { drawn, drawnAt, tooSoon, readPlaces } = useSettledPlace<HTMLElement>(
    [
      above,
      line,
      ...same.placed.map((one) => heldKey(one.ask, undefined)),
      open ? "open" : "",
    ].join("\n"),
    () => seenAttached.current?.(),
  );
  const [isNew, seenDrawnNow] = useSeenRow(askSeenKey(first), drawn, seen);
  useLayoutEffect(() => {
    seenAttached.current = seenDrawnNow;
  });
  const mayAct = () => {
    if (!tooSoon()) return true;
    setSaid(MOVED_JUST_NOW);
    return false;
  };
  const oldest = Math.min(...same.placed.map((one) => ageOf(plane, one.ask)[0]));

  /** Each chat's ask answered once with the narrowest grant, one after another. */
  const allowAll = async () => {
    if (busy) return;
    setBusy(true);
    setSaid(undefined);
    let allowed = 0;
    let refused: { who: string; why: string } | undefined;
    try {
      for (const { ask } of same.placed) {
        const option = ask.options.find((one) => one.id === same.grant.id) ?? same.grant;
        const done = await answerTaken(plane, ask, option.id).catch((err: unknown) => ({
          refused: `purlis could not send it: ${String(err)}`,
          live: false,
        }));
        if (done.refused !== undefined) {
          refused ??= { who: chainSaid(ask.chain), why: done.refused };
          continue;
        }
        allowed += 1;
        noteAnswered(plane, {
          at: Date.now(),
          chain: ask.chain,
          says: ask.says,
          answer: option.label,
        });
        onAnswered?.(ask, option, done.live);
      }
      if (refused !== undefined)
        setSaid(allowedSome(allowed, asks.length, refused.who, refused.why));
    } finally {
      setBusy(false);
      asksMoved(plane);
    }
  };

  return (
    <MessageRow
      as="row"
      id={key}
      textValue={title}
      tone={toneOfKind(kindOfAsk(first.source))}
      icon={<KindIcon kind={kindOfAsk(first.source)} />}
      ref={drawnAt}
      onDrawn={() => {
        readPlaces();
        seenDrawnNow();
      }}
      isNew={isNew}
      className="inbox-ask inbox-same"
      data-source={first.source}
      aria-labelledby={`${id}-title`}
      aria-describedby={`${id}-whose ${id}-standing`}
    >
      <MessageTop
        title={
          <span id={`${id}-title`} className="inbox-question">
            {title}
          </span>
        }
        time={<RowAge when={oldest} now={now} since="Waiting since" />}
      />
      <MessageMeta>
        <span className="inbox-who">{who}</span>
        {" · "}
        <MessageState id={`${id}-whose`}>{SOURCE_SAID[first.source]}</MessageState>
      </MessageMeta>
      <AskStanding id={`${id}-standing`} why={same.placed[0].why} now={now} />
      <MessageBody>
        <Tooltip tip={line}>
          <span className="inbox-primary ui-text-body ui-text-strong">{line}</span>
        </Tooltip>
        <p className="inbox-secondary ui-text-secondary">{forAllSaid(same.grant)}</p>
      </MessageBody>
      <AskDecision
        id={`${id}-form`}
        who={title}
        offers={[{ ...same.grant, label: ALLOW_FOR_ALL, preselected: null }]}
        grants={ALLOW_OR_KEEP.has(first.source)}
        busy={busy}
        reason={false}
        mayAct={mayAct}
        onOpen={() => {}}
        onAnswer={() => void allowAll()}
      >
        <Button
          size="sm"
          variant="secondary"
          // **Keyboard triage lands here** (#6016): the group's answer that grants nothing, with
          // Allow for all one ← away, pressed on purpose (`asksTriage.ts`'s `leadOf`).
          data-triage-lead=""
          aria-expanded={open}
          aria-label={`${ANSWER_EACH}: ${who}`}
          onPress={() => onOpen(!open)}
        >
          {ANSWER_EACH}
        </Button>
      </AskDecision>
      {said !== undefined && (
        <p className="inbox-refused" role="status">
          {said}
        </p>
      )}
    </MessageRow>
  );
}
