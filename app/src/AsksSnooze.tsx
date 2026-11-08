// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import type { Shown } from "./bindings";
import { counted } from "./dispatches";
import { DismissUndo, useDismissHold } from "./dismissHold";
import { Notice } from "./Notice";
import { Button, ButtonRow } from "./ui/button/Button";

/**
 * **Snooze all on Asks** (#5647, ADR 0119): every listed chat's asks put away at once, answering
 * nothing and with no timer. A chat comes back, with all its asks, as soon as it raises an ask
 * it did not have when snoozed: the core's inbox store keeps what each chat held, on this
 * machine, and lists a snoozed chat's asks apart (`Asking.snoozed`), so neither the ✋ nor Asks
 * counts them while the Chats view, which reads the board, still says the chat waits.
 *
 * Asked once, in a line inside the view and never a system dialog; an Undo stands for
 * `UNDO_MS` (`undo.ts`) after (the hold a Dismiss's Undo stands in, `dismissHold.tsx`, its Undo
 * waking the chats again); and while anything is snoozed the view says how many, with Show them
 * now.
 */

/** The press that asks first. */
export const SNOOZE_ALL = "Snooze all";

/** One chat Snooze all puts away: its session, and the asks it holds now, by key. */
export type SnoozedChat = { session: number; asks: string[] };

/** What the window can do with snoozed asks: each answers what went wrong, or nothing. */
export type Snoozing = {
  /** The asks Snooze all put away, as the core lists them apart. */
  snoozed: readonly Shown[];
  /** Puts each chat away with the asks it holds. */
  snooze: (chats: SnoozedChat[]) => Promise<string | undefined>;
  /** Brings back the chats `sessions`, or every snoozed chat where it is left out. */
  wake: (sessions?: number[]) => Promise<string | undefined>;
};

const asks = (count: number) => counted(count, "ask", "asks");

/** Each chat of `listed`, with the keys of the asks it holds, in the order they are listed. */
export function chatsOf(listed: readonly Shown[]): SnoozedChat[] {
  const chats = new Map<number, string[]>();
  for (const one of listed) chats.set(one.session, [...(chats.get(one.session) ?? []), one.ask]);
  return [...chats].map(([session, held]) => ({ session, asks: held }));
}

/** The Snooze all line at the head of Asks: the press, its question, and its Undo. */
export function SnoozeAll({ listed, snoozing }: { listed: readonly Shown[]; snoozing: Snoozing }) {
  const [asking, setAsking] = useState(false);
  const [trouble, setTrouble] = useState<string | undefined>();
  /** The Undo of what was just snoozed: written at once, so its Undo wakes the chats again. */
  const undo = useDismissHold();

  const takeBack = async (sessions?: number[]) => {
    setTrouble(await snoozing.wake(sessions));
  };
  const snooze = async () => {
    const chats = chatsOf(listed);
    setAsking(false);
    const failed = await snoozing.snooze(chats);
    setTrouble(failed);
    if (failed !== undefined) return;
    const sessions = chats.map((one) => one.session);
    undo.hold(
      chats.flatMap((one) => one.asks),
      () => {},
      () => void takeBack(sessions),
    );
  };

  return (
    <div className="inbox-snooze">
      {trouble !== undefined && (
        <Notice cause="asks-snooze-trouble" tone="trouble" onDismiss={() => setTrouble(undefined)}>
          {trouble}
        </Notice>
      )}
      {undo.undoable !== undefined ? (
        <DismissUndo hold={undo} cause="asks-snoozed" said={(count) => `Snoozed ${asks(count)}.`} />
      ) : asking ? (
        <div className="inbox-snooze-line" role="group" aria-label={SNOOZE_ALL}>
          <p className="inbox-none">
            {`Snooze ${asks(listed.length)}? Each chat comes back when it asks something new.`}
          </p>
          <ButtonRow size="sm">
            <Button variant="primary" onPress={() => void snooze()}>
              {SNOOZE_ALL}
            </Button>
            <Button onPress={() => setAsking(false)}>Cancel</Button>
          </ButtonRow>
        </div>
      ) : (
        listed.length > 0 && (
          <Button size="sm" variant="quiet" onPress={() => setAsking(true)}>
            {SNOOZE_ALL}
          </Button>
        )
      )}
      {snoozing.snoozed.length > 0 && undo.undoable === undefined && (
        <div className="inbox-snooze-line">
          <p className="inbox-none">
            {`${asks(snoozing.snoozed.length)} snoozed until their chats ask again.`}
          </p>
          <Button size="sm" onPress={() => void takeBack()}>
            Show them now
          </Button>
        </div>
      )}
    </div>
  );
}
