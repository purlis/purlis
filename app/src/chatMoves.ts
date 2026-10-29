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
import type { State } from "./chatState";

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

/** Every chat move of `plane`, by session. */
export function useChatMoves(plane: PlaneId): Readonly<Record<number, ChatMove>> {
  return useSyncExternalStore(subscribe, () => held[plane] ?? NONE);
}

/** Every chat move of `plane` as held now, outside React. */
export function chatMovesOf(plane: PlaneId): Readonly<Record<number, ChatMove>> {
  return held[plane] ?? NONE;
}

/** `told`, a stage the core said, held for its chat. */
export function heardMove(plane: PlaneId, told: ChatMove) {
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
    heardMove(plane, { session, from, to, stage: "failed", why: said.error, with: [] });
}

/** Puts chat `session`'s move away: one still prepared is forgotten, a failed one dismissed. */
export function putAwayMove(plane: PlaneId, session: number) {
  forgetMove(plane, session);
  void commands.putAwayChatMove(plane, session).catch(() => undefined);
}

/** The chats whose move switches now: prepared, and at a pause between turns (`waiting`). */
export function movesDue(
  moves: Readonly<Record<number, ChatMove>>,
  stateOf: (session: number) => State,
): number[] {
  return Object.values(moves)
    .filter((one) => one.stage === "ready" && stateOf(one.session) === "waiting")
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
    case "ready":
      return {
        words:
          others.length === 0
            ? `Moves to ${where} at its next pause.`
            : `Moves to ${where} with ${listed(others)} once each is at a pause.`,
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
 * **The other chats that move with chat `session`** (#3453): every other chat of its
 * workspace running where it runs, by the name its tab shows, since a workspace's chats share
 * its clones and move together. None for a chat at the project's root.
 */
export function movesWith(
  session: number,
  open: readonly Pick<OpenChat, "session" | "workspace" | "pool" | "label" | "name">[],
): string[] {
  const chat = open.find((one) => one.session === session);
  const workspace = chat?.workspace ?? null;
  if (chat === undefined || workspace === null) return [];
  const on = chat.pool ?? null;
  return open
    .filter((one) => one.session !== session)
    .filter((one) => (one.workspace ?? null) === workspace && (one.pool ?? null) === on)
    .map((one) => one.label ?? one.name);
}

/**
 * **The pools a chat may move to** (D-2048-13): every pool this machine keeps, where its runners
 * are shown to be the person's own (a Docker pool on this machine's own engine, until a pool can
 * be marked as yours, #2561), and none otherwise. A chat's conversation and work go nowhere else.
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
