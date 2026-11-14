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
 * - **A window-only ask no chat waits on sits here** (T1-42, R-329, Q181): the core lists it
 *   apart from Asks (`Asking.on_finish_rows`) once its chat's turn has ended, and it is drawn on
 *   that chat's newest finish row, its words under the line and its control beside them
 *   ({@link finishRowsOf}). A chat with no finish counted yet gets one, so the ask is never
 *   drawn nowhere. Such a row stands alone, never grouped with another chat's.
 */
import { useEffect, useState } from "react";
import {
  commands,
  type FinishedBlock,
  type FinishedTurn,
  type InboxUpdate,
  type PlaneId,
  type Shown,
  type UpdateNoted,
} from "./bindings";
import { answerThrough } from "./asks";
import type { ChatStates, Chats } from "./chatState";
import { OPEN_THE_CHAT_LOG, openTheChatLogOf } from "./chatLog";
import type { UpdateAnswer } from "./InboxUpdatesList";
import { waysOut } from "./blockWays";

/** What a finish row adds where its turn ended with blocks still unresolved (Q164). */
export const BLOCKED_THIS_TURN = (blocked: number) => `· ${blocked} blocked this turn`;

/** What a finished turn's row says. */
export const FINISHED_ITS_TURN = "Finished its turn";

/** What a failed chat's row says. */
export const FAILED = "Failed";

/** The end of a finish row's key whose turn ended after a block. */
const BLOCKED_KEY = ":blocked";

/**
 * **The key of `turn`'s row**: its chat's number, the event that ended it, and whether a block
 * stopped it. The event is the chat's own (the core finds it through the chat's logged id), so
 * a kept row whose key no finish answered now has is of a chat not open here under that number.
 */
export const finishKey = (turn: FinishedTurn) =>
  `finish:${turn.session}:${turn.key}${!turn.failed && turn.blocked > 0 ? BLOCKED_KEY : ""}`;

/**
 * **Which chats a finish row's ways out act on** (Q272): Tell the chat… types only into the
 * chats whose own turn holds a block that offers it, never into every chat of a grouped row,
 * and Go to chat goes to the chat that holds the Can't-tell block. Each chat once, in the
 * order the turns come.
 */
export function finishWaysTo(
  turns: readonly FinishedTurn[],
  keys: readonly string[],
): { tell: number[]; go: number | undefined } {
  const these = new Set(keys);
  const tell: number[] = [];
  let go: number | undefined;
  for (const turn of turns) {
    if (turn.failed || !these.has(keyOf(turn))) continue;
    const ways = waysOut(turn.blocks);
    if (ways.includes("tell") && !tell.includes(turn.session)) tell.push(turn.session);
    if (ways.includes("go") && go === undefined) go = turn.session;
  }
  return { tell, go };
}

/**
 * **`turns`, with a finish for each chat of `asks` that has none counted** (R-329): its turn
 * ended, or the core would not have listed its ask apart, so the ask has a row to sit on.
 */
function withAnEndFor(
  turns: readonly FinishedTurn[],
  asks: readonly Shown[],
): readonly FinishedTurn[] {
  const counted = new Set(turns.filter((turn) => !turn.failed).map((turn) => turn.session));
  const added = new Map<number, FinishedTurn>();
  for (const ask of asks)
    if (!counted.has(ask.session) && !added.has(ask.session))
      added.set(ask.session, {
        session: ask.session,
        key: `ask-${ask.ask}`,
        at: ask.since ?? Math.floor(Date.now() / 1000),
        failed: false,
        blocked: 0,
        blocks: [],
      });
  return added.size === 0 ? turns : [...turns, ...added.values()];
}

/** **The finish rows of `turns`**, each named by its chat, and one for each chat of `asks`
 *  that has none counted (R-329). */
export function finishUpdates(
  turns: readonly FinishedTurn[],
  nameOf: (session: number) => string,
  asks: readonly Shown[] = [],
): UpdateNoted[] {
  return withAnEndFor(turns, asks).map((turn) => ({
    key: finishKey(turn),
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

/** What a window-only ask adds to the finish row it sits on: its words, and its answers. */
export type OnAFinishRow = { more: string; answers: readonly UpdateAnswer[] };

/**
 * **The window-only asks on each finish row, by the row's key** (T1-42, R-329): each ask on its
 * chat's newest finish ({@link withAnEndFor} gives one to a chat with none), its words under the
 * line and each of its options an answer, sent through the path the ask names
 * ({@link answerThrough}), the same press its row in Asks makes. An answer that allows waits for
 * the row to settle, as every update's does (`InboxUpdatesList`). `reread` reads the asks again
 * once it is answered.
 */
export function finishAsksOf({
  plane,
  turns,
  asks,
  reread,
}: {
  plane: PlaneId;
  turns: readonly FinishedTurn[];
  asks: readonly Shown[];
  reread: () => void;
}): ReadonlyMap<string, OnAFinishRow> {
  const newest = new Map<number, FinishedTurn>();
  for (const turn of withAnEndFor(turns, asks)) {
    if (turn.failed) continue;
    const was = newest.get(turn.session);
    if (was === undefined || turn.at >= was.at) newest.set(turn.session, turn);
  }
  const on = new Map<string, { more: string[]; answers: UpdateAnswer[] }>();
  for (const ask of asks) {
    const turn = newest.get(ask.session);
    if (turn === undefined) continue;
    const key = finishKey(turn);
    const row = on.get(key) ?? { more: [], answers: [] };
    const what = ask.answer.via === "trust-clone" ? ask.answer.folder : ask.says;
    row.more.push(ask.says);
    for (const option of ask.options)
      row.answers.push({
        label: option.label,
        name: `${option.label}: ${what}`,
        title: TRUSTS_IT,
        allows: option.allows,
        // A refusal is said on the row (Q274), as an ask's row says it.
        press: () =>
          answerThrough(plane, ask, option.id)
            .catch((err: unknown) => String(err))
            .finally(reread),
      });
    on.set(key, row);
  }
  // Two asks on one row would draw two buttons alike (Q274): each then shows its whole name.
  return new Map(
    [...on].map(([key, row]) => {
      const labels = row.answers.map((answer) => answer.label);
      const answers = row.answers.map((answer) =>
        labels.indexOf(answer.label) === labels.lastIndexOf(answer.label)
          ? answer
          : { ...answer, label: answer.name },
      );
      return [key, { more: row.more.join(" "), answers }];
    }),
  );
}

/** What Trust this clone does, as its tooltip on a finish row. */
const TRUSTS_IT =
  "Record this clone as yours, so chats in its workspace can use your GitHub access with it.";

/**
 * **The finish rows of `turns`, with the window-only asks no chat waits on** (T1-42, R-329):
 * each row as {@link finishUpdates} makes it, and where an ask of `asks` sits on it, its words
 * and its control ({@link finishAsksOf}).
 */
export function finishRowsOf({
  plane,
  turns,
  asks,
  nameOf,
  reread,
}: {
  plane: PlaneId;
  turns: readonly FinishedTurn[];
  asks: readonly Shown[];
  nameOf: (session: number) => string;
  reread: () => void;
}): ({ update: UpdateNoted } & Partial<OnAFinishRow>)[] {
  const on = finishAsksOf({ plane, turns, asks, reread });
  return finishUpdates(turns, nameOf, asks).map((update) => ({
    update,
    ...on.get(update.key),
  }));
}

/** **Whether `update` is a finish row whose turn ended after a block** (R-182). */
export const stoppedAfterBlock = (update: Pick<InboxUpdate, "kind" | "key">): boolean =>
  update.kind === "turn-finished" && update.key.endsWith(BLOCKED_KEY);

/** What every block result's way to its Chat log says, whatever chat it names (R-226). */
export const CHAT_LOG_ANSWER = {
  label: OPEN_THE_CHAT_LOG,
  title: "See what was blocked, in its chat log.",
} as const;

/**
 * **A block result's way to its chat's Chat log** (R-226, T1-37): the answer of a finish row
 * whose turn ended after a block ({@link stoppedAfterBlock}), naming the chat called `name`.
 * `open` opens that chat's Chat log at its blocks.
 */
export function chatLogAnswer(name: string, open: () => void): UpdateAnswer {
  return {
    ...CHAT_LOG_ANSWER,
    name: openTheChatLogOf(name),
    press: () => {
      open();
      return undefined;
    },
  };
}

/** A row as Updates lists it, and every update it stands for. */
export type Grouped = { update: InboxUpdate; keys: readonly string[] };

/**
 * **`updates` with identical finish rows grouped across chats** (R-181), in their order: the
 * finished turns of several chats that say the same and are read alike are one row, where the
 * newest of them stands, naming each chat once. Any other update stands alone.
 */
export function groupedFinishes(
  updates: readonly InboxUpdate[],
  /** The rows that stand alone: each finish row a window-only ask sits on (R-329). */
  apart: ReadonlySet<string> = new Set(),
): Grouped[] {
  const out: { update: InboxUpdate; keys: string[]; names: string[] }[] = [];
  const groups = new Map<string, (typeof out)[number]>();
  for (const update of updates) {
    if (update.kind !== "turn-finished" || apart.has(update.key)) {
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

/** **Each chat's last end**: when it last moved to an end, or `undefined` while its turn runs. */
const endsOf = (states: ChatStates): Map<number, number | undefined> =>
  new Map(
    Object.entries(states.bySession).map(([session, state]) => [
      Number(session),
      ENDED.has(state) ? (states.movedAt[Number(session)] ?? 0) : undefined,
    ]),
  );

const NONE: readonly FinishedTurn[] = [];

/**
 * **The finishes of every chat of `plane` the window knows of** (R-178), as the core counts
 * them (`finished_turns`): asked again only when a chat's turn ends (it moves to an end it was
 * not at), never on a poll and never on a move within a turn. **It listens to the chats outside
 * React**, so a move redraws nothing here (`MovedRenders`): only an answer that differs from the
 * last one does. **The read waits for the next task**, so a project that comes forward paints
 * before it is asked, and the ends of one burst are one read. An answer for another project, or
 * one that is no list (an older build), is none.
 *
 * **Only the newest answer is kept** (Q263): each read is numbered, and one that answers after
 * a later read has answered is dropped. **A read that failed is asked again when a turn next
 * ends**, whichever chat's.
 */
export function useFinishedTurns(plane: PlaneId, chats: Chats): readonly FinishedTurn[] {
  const { store } = chats;
  const [held, setHeld] = useState<{ plane: PlaneId; turns: readonly FinishedTurn[] }>();
  useEffect(() => {
    let live = true;
    /** Each chat's end as last seen; a chat first seen at an end is a turn that ended. */
    let seen = new Map<number, number | undefined>();
    let said = "";
    /** The read waiting for the next task, if one is. */
    let pending: ReturnType<typeof setTimeout> | undefined;
    /** The number of the last read sent, and of the newest one that has answered. */
    let sent = 0;
    let answered = 0;
    const read = () => {
      pending = undefined;
      if (!live) return;
      const sessions = [...seen.keys()];
      if (sessions.length === 0) return;
      const which = ++sent;
      void commands
        .finishedTurns(plane, sessions)
        .then((answer) => {
          if (!live || which < answered) return;
          if (answer.status !== "ok" || !Array.isArray(answer.data)) return;
          answered = which;
          const now = JSON.stringify(answer.data);
          if (now === said) return;
          said = now;
          setHeld({ plane, turns: answer.data });
        })
        .catch(() => undefined);
    };
    const look = () => {
      const now = endsOf(store.statesFor(chats.plane));
      const ended = [...now].some(([session, at]) => at !== undefined && seen.get(session) !== at);
      seen = now;
      if (ended && pending === undefined) pending = setTimeout(read, 0);
    };
    look();
    const stop = store.subscribe(look);
    return () => {
      live = false;
      if (pending !== undefined) clearTimeout(pending);
      stop();
    };
  }, [plane, store, chats.plane]);
  return held?.plane === plane ? held.turns : NONE;
}

/**
 * **What the finish rows `keys` offer ways out of** (T1-33, R-300): the unresolved blocks of
 * the turns they stand for, each once, a grouped row's every chat's. A failed run's row, or a
 * turn with no block left, offers none.
 */
export function finishWaysOf(
  turns: readonly FinishedTurn[],
  keys: readonly string[],
): FinishedBlock[] {
  const these = new Set(keys);
  const seen = new Set<string>();
  const out: FinishedBlock[] = [];
  for (const turn of turns) {
    if (turn.failed || !these.has(keyOf(turn))) continue;
    for (const block of turn.blocks) {
      const said = JSON.stringify([
        block.source,
        block.qualifier,
        block.target_kind,
        block.target,
        block.harness,
      ]);
      if (seen.has(said)) continue;
      seen.add(said);
      out.push(block);
    }
  }
  return out;
}
