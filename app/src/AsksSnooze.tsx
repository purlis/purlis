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
 * nothing. A chat comes back, with all its asks, as soon as it raises an ask it did not have
 * when snoozed or, where the person chose **For 1 hour** or **Until tomorrow** (09:00 local time
 * the next day), when that time comes, whichever is first (#6018, ADR 0121 decision 14). The
 * core's inbox store keeps what each chat held, and until when, on this machine, and lists a
 * snoozed chat's asks apart (`Asking.snoozed`), so neither the ✋ nor Asks counts them, and no
 * notification is sent for them, while the Chats view, which reads the board, still says the
 * chat waits.
 *
 * Asked once, in a line inside the view and never a system dialog; an Undo stands for
 * `UNDO_MS` (`undo.ts`) after (the hold a Dismiss's Undo stands in, `dismissHold.tsx`, its Undo
 * waking the chats again); and while anything is snoozed the view says how many, and until
 * when, with Show them now.
 */

/** The press that asks first. */
export const SNOOZE_ALL = "Snooze all";

/** One chat Snooze all puts away: its session, and the asks it holds now, by key. */
export type SnoozedChat = { session: number; asks: string[] };

/** **How long a snooze lasts at most** (#6018): until the chat asks something new and nothing
 *  else, or a time as well. */
export type SnoozeFor = "new-ask" | "hour" | "tomorrow";

/** **When a snooze chosen at `now` ends by itself**, in seconds since 1970: an hour on, or 09:00
 *  local time the next day; none for a snooze until the chat asks something new. */
export function snoozeEnds(chosen: SnoozeFor, now: Date): number | null {
  if (chosen === "new-ask") return null;
  const at =
    chosen === "hour"
      ? new Date(now.getTime() + 60 * 60 * 1000)
      : new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1, 9, 0, 0);
  return Math.floor(at.getTime() / 1000);
}

/** **When snoozed asks come back, as the view says it**: the time today, "tomorrow at" a time,
 *  or the date and time further on. `until` is in seconds since 1970. */
export function backAtSaid(until: number, now: Date): string {
  const at = new Date(until * 1000);
  const time = at.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  const day = (one: Date) => new Date(one.getFullYear(), one.getMonth(), one.getDate()).getTime();
  const tomorrow = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1).getTime();
  if (day(at) === day(now)) return time;
  if (day(at) === tomorrow) return `tomorrow at ${time}`;
  return at.toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** What the window can do with snoozed asks: each answers what went wrong, or nothing. */
export type Snoozing = {
  /** The asks Snooze all put away, as the core lists them apart. */
  snoozed: readonly Shown[];
  /** When the first of them comes back by itself, in seconds since 1970, if any has a time. */
  snoozedUntil?: number | null;
  /** Puts each chat away with the asks it holds, until `until` at the latest where given. */
  snooze: (chats: SnoozedChat[], until: number | null) => Promise<string | undefined>;
  /** Brings back the chats `sessions`, or every snoozed chat where it is left out. */
  wake: (sessions?: number[]) => Promise<string | undefined>;
};

const asks = (count: number) => counted(count, "ask", "asks");

/** What the Undo line says after "Snoozed 2 asks" for each choice. */
const FOR_SAID: Record<SnoozeFor, string> = {
  "new-ask": "",
  hour: " for 1 hour",
  tomorrow: " until tomorrow",
};

/** Each chat of `listed`, with the keys of the asks it holds, in the order they are listed. */
export function chatsOf(listed: readonly Shown[]): SnoozedChat[] {
  const chats = new Map<number, string[]>();
  for (const one of listed) chats.set(one.session, [...(chats.get(one.session) ?? []), one.ask]);
  return [...chats].map(([session, held]) => ({ session, asks: held }));
}

/** The Snooze all line at the head of Asks: the press, its question, and its Undo. */
export function SnoozeAll({ listed, snoozing }: { listed: readonly Shown[]; snoozing: Snoozing }) {
  const [asking, setAsking] = useState(false);
  /** How long the last snooze was chosen for: what its Undo line says. */
  const [chosen, setChosen] = useState<SnoozeFor>("new-ask");
  const [trouble, setTrouble] = useState<string | undefined>();
  /** The Undo of what was just snoozed: written at once, so its Undo wakes the chats again. */
  const undo = useDismissHold();

  const takeBack = async (sessions?: number[]) => {
    setTrouble(await snoozing.wake(sessions));
  };
  const snooze = async (choice: SnoozeFor) => {
    const chats = chatsOf(listed);
    setAsking(false);
    setChosen(choice);
    const failed = await snoozing.snooze(chats, snoozeEnds(choice, new Date()));
    setTrouble(failed);
    if (failed !== undefined) return;
    const sessions = chats.map((one) => one.session);
    undo.hold(
      chats.flatMap((one) => one.asks),
      () => {},
      () => void takeBack(sessions),
    );
  };
  const until = snoozing.snoozedUntil ?? null;

  return (
    <div className="inbox-snooze">
      {trouble !== undefined && (
        <Notice cause="asks-snooze-trouble" tone="trouble" onDismiss={() => setTrouble(undefined)}>
          {trouble}
        </Notice>
      )}
      {undo.undoable !== undefined ? (
        <DismissUndo
          hold={undo}
          cause="asks-snoozed"
          said={(count) => `Snoozed ${asks(count)}${FOR_SAID[chosen]}.`}
        />
      ) : asking ? (
        <div className="inbox-snooze-line" role="group" aria-label={SNOOZE_ALL}>
          <p className="inbox-none">
            {`Snooze ${asks(listed.length)}? Each chat comes back when it asks something new, or at the time you choose if that comes first.`}
          </p>
          <ButtonRow size="sm">
            <Button variant="primary" onPress={() => void snooze("new-ask")}>
              {SNOOZE_ALL}
            </Button>
            <Button onPress={() => void snooze("hour")}>For 1 hour</Button>
            <Button onPress={() => void snooze("tomorrow")}>Until tomorrow</Button>
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
            {until === null
              ? `${asks(snoozing.snoozed.length)} snoozed until their chats ask again.`
              : `${asks(snoozing.snoozed.length)} snoozed until ${backAtSaid(until, new Date())}, or until their chats ask again.`}
          </p>
          <Button size="sm" onPress={() => void takeBack()}>
            Show them now
          </Button>
        </div>
      )}
    </div>
  );
}
