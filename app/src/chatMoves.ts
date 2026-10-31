// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useSyncExternalStore } from "react";
import { listen } from "./here";
import {
  commands,
  type ChatMove,
  type MoveTold,
  type OpenChat,
  type PlaneId,
  type PoolsShown,
} from "./bindings";
import { atAPause, type ChatStates } from "./chatState";

/**
 * **Move to runner and Move here, as the window holds them** (#2048): each chat's move by
 * project, as the core tells its stages (`chat-move`) and as it is read when a project is drawn
 * anew (`chat_moves`). Its pane says each stage ({@link moveSaid}) and its tab's menu offers the
 * move ({@link moveOffers}).
 *
 * **The switch waits for the chat's pause between turns** ({@link movesDue}), as a restart owed
 * does: the core prepares the move while the chat keeps working, and the window asks it to
 * switch (`move_chat_now`) once the move is ready and the chat's turn has ended. The core asks
 * the board again, so a move is never made mid-turn whoever asks.
 */

type ByPlane = Readonly<Record<string, Readonly<Record<number, ChatMove>>>>;

let held: ByPlane = {};
const listeners = new Set<() => void>();

function set(
  plane: PlaneId,
  change: (was: Readonly<Record<number, ChatMove>>) => Record<number, ChatMove>,
) {
  held = { ...held, [plane]: change(held[plane] ?? {}) };
  for (const one of listeners) one();
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

const NONE: Readonly<Record<number, ChatMove>> = {};

/**
 * **The moves that waited on a sign-in, by chat, and the pool each goes to** (#4135): a move that
 * then goes ahead by itself can be missed, so its chat says it moved. Taken when the move is made
 * ({@link movedAfterSignIn}). A move put away, failed or asked anew leaves nothing here; one past
 * cancelling stays, whatever its line shows, as its answer may come after its `over`.
 */
const waitedOnASignIn = new Map<string, string>();
const waitKey = (plane: PlaneId, session: number) => `${plane}\0${session}`;

/** **The chats that moved after waiting on a sign-in**, each with its pool, until dismissed. */
let movedAfterWait: Readonly<Record<string, Readonly<Record<number, string>>>> = {};
const NOT_MOVED: Readonly<Record<number, string>> = {};

/** Every chat move of `plane`, by session. */
export function useChatMoves(plane: PlaneId): Readonly<Record<number, ChatMove>> {
  return useSyncExternalStore(subscribe, () => held[plane] ?? NONE);
}

/** Every chat move of `plane` as held now, outside React. */
export function chatMovesOf(plane: PlaneId): Readonly<Record<number, ChatMove>> {
  return held[plane] ?? NONE;
}

/**
 * `told`, a stage the core said, held for its chat. `over` is forgotten: the move was made or
 * put away, from this tab or another of its workspace (#3814).
 */
export function heardMove(plane: PlaneId, told: ChatMove) {
  const key = waitKey(plane, told.session);
  if (told.stage === "over") {
    // `over` says the same of a move made and one put away. Only these stages can be put away;
    // with none held, the line was dismissed or put away here, which says so itself.
    const was = held[plane]?.[told.session]?.stage;
    if (was === "preparing" || was === "signing-in" || was === "ready")
      waitedOnASignIn.delete(key);
    forgetMove(plane, told.session);
    return;
  }
  const had = held[plane]?.[told.session];
  if (told.stage === "signing-in" && told.to !== null) waitedOnASignIn.set(key, told.to);
  else if (
    told.stage === "failed" ||
    told.to !== waitedOnASignIn.get(key) ||
    // A move asked anew: one that waited comes here from its wait.
    (had === undefined && told.stage !== "switching")
  )
    waitedOnASignIn.delete(key);
  set(plane, (was) => ({ ...was, [told.session]: told }));
}

/** Chat `session`'s move is over: it moved, or it was put away. */
export function forgetMove(plane: PlaneId, session: number) {
  set(plane, (was) => {
    if (!(session in was)) return was;
    return Object.fromEntries(
      Object.entries(was).filter(([one]) => Number(one) !== session),
    ) as Record<number, ChatMove>;
  });
}

/**
 * **Chat `was` moved and is now chat `now`** (#4135): where its move waited on a sign-in, the chat
 * says it moved, since it went ahead by itself.
 */
export function movedAfterSignIn(plane: PlaneId, was: number, now: number) {
  const key = waitKey(plane, was);
  const pool = waitedOnASignIn.get(key);
  if (pool === undefined) return;
  waitedOnASignIn.delete(key);
  movedAfterWait = {
    ...movedAfterWait,
    [plane]: { ...movedAfterWait[plane], [now]: pool },
  };
  for (const one of listeners) one();
}

/** The pool chat `session` moved to after waiting on a sign-in, until that is dismissed. */
export function useMovedAfterSignIn(plane: PlaneId, session: number): string | undefined {
  return useSyncExternalStore(subscribe, () => (movedAfterWait[plane] ?? NOT_MOVED)[session]);
}

/** Dismisses what chat `session` says of having moved. */
export function dismissMovedAfterSignIn(plane: PlaneId, session: number) {
  const left = Object.entries(movedAfterWait[plane] ?? NOT_MOVED).filter(
    ([one]) => Number(one) !== session,
  );
  movedAfterWait = { ...movedAfterWait, [plane]: Object.fromEntries(left) };
  for (const one of listeners) one();
}

/**
 * **Hears `plane`'s moves**: read once as the project is drawn, then each stage as the core
 * tells it. Called once per drawn project.
 */
export function useChatMovesHeard(plane: PlaneId) {
  useEffect(() => {
    let gone = false;
    let stop: (() => void) | undefined;
    void commands
      .chatMoves(plane)
      .then((read) => {
        if (gone || read.status !== "ok" || !Array.isArray(read.data)) return;
        for (const one of read.data) heardMove(plane, one);
      })
      .catch(() => undefined);
    void (async () => {
      try {
        const unlisten = await listen<MoveTold>("chat-move", (event) => {
          const told = event.payload;
          if (gone || told.plane !== plane) return;
          heardMove(plane, told.moving);
        });
        if (gone) unlisten();
        else stop = unlisten;
      } catch {
        // No window to listen in.
      }
    })();
    return () => {
      gone = true;
      stop?.();
    };
  }, [plane]);
}

/**
 * **Asks for chat `session`, running on pool `from` or here for `null`, to move to pool `to`, or
 * here for `null`**, with every chat of its workspace running where it runs (#3453): the core
 * checks it now and prepares it. Its refusal is held as a failed move, which the pane says.
 */
export async function askMove(
  plane: PlaneId,
  session: number,
  from: string | null,
  to: string | null,
) {
  const said = await commands
    .moveChat(plane, session, to)
    .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
  if (said.status === "error")
    heardMove(plane, {
      session,
      from,
      to,
      stage: "failed",
      why: said.error,
      with: [],
      sign_in: null,
    });
}

/**
 * **The core's moves, read back after it refused a button** (#4138): each it holds is held here,
 * and a move here it no longer holds is forgotten, say one whose `over` the window missed. A
 * failed move stays: a refusal the window heard is one the core may never have held, and it stays
 * until it is dismissed. A read that failed changes nothing.
 */
async function readBackMoves(plane: PlaneId) {
  const read = await commands.chatMoves(plane);
  if (read.status !== "ok") return;
  const holds = new Set(read.data.map((one) => one.session));
  for (const one of Object.values(chatMovesOf(plane)))
    if (one.stage !== "failed" && !holds.has(one.session)) {
      waitedOnASignIn.delete(waitKey(plane, one.session));
      forgetMove(plane, one.session);
    }
  for (const one of read.data) heardMove(plane, one);
}

/**
 * Puts chat `session`'s move away: one still prepared is forgotten, for every chat of it as the
 * core tells each (`over`), and a failed one dismissed. One the core says is being made already
 * is read back, so its tab says so again (#3814).
 */
export function putAwayMove(plane: PlaneId, session: number) {
  forgetMove(plane, session);
  void commands
    .putAwayChatMove(plane, session)
    .then(async (done) => {
      // Refused, it is being made already, and goes on to say it moved.
      if (done.status === "ok") {
        waitedOnASignIn.delete(waitKey(plane, session));
        return;
      }
      await readBackMoves(plane);
    })
    .catch(() => undefined);
}

/**
 * **Chat `session`'s move waited on its profile's sign-in, which is done** (#3477): the core
 * prepares it again, asking the runner itself, so it goes ahead by itself once signed in there.
 * Where the core refused, its moves are read back, so the pane says what the core holds (another
 * tab of the move may have told it first).
 */
export function signedInForMove(plane: PlaneId, session: number) {
  void commands
    .chatMoveSignedIn(plane, session)
    .then(async (said) => {
      if (said.status === "ok") return;
      await readBackMoves(plane);
    })
    .catch(() => undefined);
}

/** Whether a move may still be cancelled: while it is prepared, waits on a sign-in or is ready,
 *  never while it is made. */
export function moveCancellable(one: ChatMove): boolean {
  return one.stage === "preparing" || one.stage === "signing-in" || one.stage === "ready";
}

/**
 * The chats whose move switches now: prepared, and at a pause between turns ({@link atAPause}). A
 * chat waiting on its own work (#3878) is not at a pause: a move would cut its monitors and
 * background shells off, so it waits for them (#3887).
 */
export function movesDue(moves: Readonly<Record<number, ChatMove>>, states: ChatStates): number[] {
  return Object.values(moves)
    .filter((one) => one.stage === "ready" && atAPause(states, one.session))
    .map((one) => one.session);
}

/** Where a move goes, in words. */
export const placeSaid = (to: string | null) => (to === null ? "this machine" : `pool ${to}`);

/** Names in a sentence: `a`, `a and b`, `a, b and c`. */
export function listed(names: readonly string[]): string {
  if (names.length <= 1) return names[0] ?? "";
  return `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}`;
}

/**
 * What chat `session`'s pane says of its move, and whether it is trouble. A chat that moves with
 * others of its workspace names them (#3453); one that did not move names where it still runs,
 * once (#3646).
 */
export function moveSaid(one: ChatMove): { words: string; trouble: boolean } {
  const where = placeSaid(one.to);
  const others = one.with ?? [];
  const meanwhile = one.to === null ? "its work comes back" : "the runner is readied";
  switch (one.stage) {
    case "preparing":
      return {
        words:
          others.length === 0
            ? `Moving to ${where}. It keeps working here while ${meanwhile}, and moves at its next pause.`
            : `Moving to ${where} with ${listed(others)}, which share its workspace's clones. Each keeps working here while ${meanwhile}, and they move together once each is at a pause.`,
        trouble: false,
      };
    case "signing-in":
      return {
        words: `Moves to ${where} once ${one.sign_in?.profile ?? "its profile"} is signed in there. ${
          others.length === 0
            ? "It keeps working here meanwhile."
            : `It and ${listed(others)} keep working here meanwhile.`
        }`,
        trouble: false,
      };
    case "ready":
      return {
        words:
          others.length === 0
            ? `Moves to ${where} at its next pause.`
            : `Moves to ${where} with ${listed(others)} once each is at a pause.`,
        trouble: false,
      };
    case "switching":
      return {
        words:
          others.length === 0
            ? `Moving to ${where} now.`
            : `Moving to ${where} now, with ${listed(others)}.`,
        trouble: false,
      };
    default:
      return {
        words:
          `Did not move to ${where}, so it still runs on ${placeSaid(one.from ?? null)}. ${one.why ?? ""}`.trim(),
        trouble: true,
      };
  }
}

/**
 * **The other chats that move with chat `session`** (#3453): every other harness chat of its
 * workspace running where it runs, by the name its tab shows, since a workspace's chats share
 * its clones and move together. None for a chat at the project's root.
 */
export function movesWith(
  session: number,
  open: readonly Pick<OpenChat, "session" | "workspace" | "pool" | "label" | "name" | "harness">[],
): string[] {
  const chat = open.find((one) => one.session === session);
  const workspace = chat?.workspace ?? null;
  if (chat === undefined || workspace === null) return [];
  const on = chat.pool ?? null;
  return (
    open
      .filter((one) => one.session !== session)
      // A shell does not move, and the core says so in its refusal (#3814).
      .filter((one) => one.harness !== null)
      .filter((one) => (one.workspace ?? null) === workspace && (one.pool ?? null) === on)
      .map((one) => one.label ?? one.name)
  );
}

/**
 * **The pools a chat may move to** (D-2048-13): every pool this machine keeps, where its runners
 * are shown to be the person's own (a Docker pool on this machine's own engine), and none
 * otherwise. A chat's conversation and work go nowhere else.
 */
export function poolsToMoveTo(shown: Pick<PoolsShown, "pools" | "yours"> | undefined): string[] {
  if (shown === undefined || shown.yours !== true) return [];
  return shown.pools.map((pool) => pool.name);
}

/**
 * **What a chat's tab offers to move it**: Move here for a chat on a pool's runner, and one Move
 * to pool row per pool this machine keeps for a harness chat here. Nothing for a shell, which
 * runs where its window composed it.
 */
export function moveOffers(
  chat: Pick<OpenChat, "harness" | "pool"> | undefined,
  pools: readonly string[],
): (string | null)[] {
  if (chat === undefined || chat.harness === null) return [];
  const on = chat.pool ?? null;
  if (on !== null) return [null];
  return [...pools];
}
