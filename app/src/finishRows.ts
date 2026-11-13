// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The turn's finish row in Updates › Mine** (T1-31, R-241, R-178 to R-182; Q131, Q164): one
 * row for each turn of a chat's that ended ("Finished its turn"), and one for each of its own
 * runs that ended failed ("Failed"). **The core decides what a turn's end says**
 * (`purlis_core::eventlog::finishes`, B2): a turn whose block was still unresolved when it ended
 * (no later success on the same target, and no Allow) gains "· 1 blocked this turn"; this window
 * only draws what it counted.
 *
 * - **One row per turn** (R-179): keyed by the event that ended it, whatever its count.
 * - **Never a failure** (R-180): a blocked turn's row is a finished turn's, in its style and
 *   its list, and no failure notification is sent of it (`notificationPrefs.SOURCES`).
 * - **Grouped across chats** (R-181): identical finish rows of several chats are one row that
 *   names them all ({@link groupedFinishes}); its Read and Dismiss settle each.
 * - **Quiet's "stopped after a block"** (R-182): {@link stoppedAfterBlock} is the bucket's test.
 */
import { useEffect, useState } from "react";
import {
  commands,
  type FinishedTurn,
  type InboxUpdate,
  type PlaneId,
  type UpdateNoted,
} from "./bindings";
import type { ChatStates, Chats } from "./chatState";

/** What a finish row adds where its turn ended with blocks still unresolved (Q164). */
export const BLOCKED_THIS_TURN = (blocked: number) => `· ${blocked} blocked this turn`;

/** What a finished turn's row says. */
export const FINISHED_ITS_TURN = "Finished its turn";

/** What a failed chat's row says. */
export const FAILED = "Failed";

/** The end of a finish row's key whose turn ended after a block. */
const BLOCKED_KEY = ":blocked";

/** The key of `turn`'s row: the event that ended it, and whether a block stopped it. */
const keyOf = (turn: FinishedTurn) =>
  `finish:${turn.session}:${turn.key}${!turn.failed && turn.blocked > 0 ? BLOCKED_KEY : ""}`;

/** **The finish rows of `turns`**, each named by its chat. */
export function finishUpdates(
  turns: readonly FinishedTurn[],
  nameOf: (session: number) => string,
): UpdateNoted[] {
  return turns.map((turn) => ({
    key: keyOf(turn),
    kind: turn.failed ? "chat-failed" : "turn-finished",
    at: turn.at,
    session: turn.session,
    chain: [nameOf(turn.session)],
    says: turn.failed
      ? FAILED
      : turn.blocked > 0
        ? `${FINISHED_ITS_TURN} ${BLOCKED_THIS_TURN(turn.blocked)}`
        : FINISHED_ITS_TURN,
  }));
}

/** **Whether `update` is a finish row whose turn ended after a block** (R-182). */
export const stoppedAfterBlock = (update: Pick<InboxUpdate, "kind" | "key">): boolean =>
  update.kind === "turn-finished" && update.key.endsWith(BLOCKED_KEY);

/** A row as Updates lists it, and every update it stands for. */
export type Grouped = { update: InboxUpdate; keys: readonly string[] };

/**
 * **`updates` with identical finish rows grouped across chats** (R-181), in their order: the
 * finished turns of several chats that say the same and are read alike are one row, where the
 * newest of them stands, naming each chat once. Any other update stands alone.
 */
export function groupedFinishes(updates: readonly InboxUpdate[]): Grouped[] {
  const out: { update: InboxUpdate; keys: string[]; names: string[] }[] = [];
  const groups = new Map<string, (typeof out)[number]>();
  for (const update of updates) {
    if (update.kind !== "turn-finished") {
      out.push({ update, keys: [update.key], names: [] });
      continue;
    }
    const same = `${update.says}\n${update.read}`;
    const name = update.chain.at(-1) ?? "";
    const group = groups.get(same);
    if (group === undefined) {
      const one = { update, keys: [update.key], names: [name] };
      groups.set(same, one);
      out.push(one);
      continue;
    }
    group.keys.push(update.key);
    if (!group.names.includes(name)) group.names.push(name);
  }
  return out.map(({ update, keys, names }) =>
    names.length > 1
      ? { update: { ...update, session: null, chain: [names.join(", ")] }, keys }
      : { update, keys },
  );
}

/** The states a chat's turn has ended in: `waiting` on the person, `done` and `failed`. */
const ENDED = new Set(["waiting", "done", "failed"]);

/** **What changes when a chat's turn ends**: each chat, and when it last moved where it moved to
 *  an end. One string, so a move within a turn asks nothing. */
const endsOf = (states: ChatStates) =>
  Object.entries(states.bySession)
    .map(([session, state]) =>
      ENDED.has(state) ? `${session}:${states.movedAt[Number(session)] ?? 0}` : session,
    )
    .join(",");

const NONE: readonly FinishedTurn[] = [];

/**
 * **The finishes of every chat of `plane` the window knows of** (R-178), as the core counts
 * them (`finished_turns`): asked again each time a chat's turn ends, never on a poll. **It
 * listens to the chats outside React**, so a move redraws nothing here (`MovedRenders`): only
 * an answer that differs from the last one does. An answer for another project, or one that is
 * no list (an older build), is none.
 */
export function useFinishedTurns(plane: PlaneId, chats: Chats): readonly FinishedTurn[] {
  const { store } = chats;
  const [held, setHeld] = useState<{ plane: PlaneId; turns: readonly FinishedTurn[] }>();
  useEffect(() => {
    let live = true;
    let asked: string | undefined;
    let said = "";
    const ask = () => {
      const ends = endsOf(store.statesFor(chats.plane));
      if (ends === asked) return;
      asked = ends;
      const sessions = ends === "" ? [] : ends.split(",").map((one) => Number(one.split(":")[0]));
      if (sessions.length === 0) return;
      void commands
        .finishedTurns(plane, sessions)
        .then((answer) => {
          if (!live || answer.status !== "ok" || !Array.isArray(answer.data)) return;
          const now = JSON.stringify(answer.data);
          if (now === said) return;
          said = now;
          setHeld({ plane, turns: answer.data });
        })
        .catch(() => undefined);
    };
    ask();
    const stop = store.subscribe(ask);
    return () => {
      live = false;
      stop();
    };
  }, [plane, store, chats.plane]);
  return held?.plane === plane ? held.turns : NONE;
}
