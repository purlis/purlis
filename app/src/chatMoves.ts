// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useSyncExternalStore } from "react";
import { listen } from "@tauri-apps/api/event";
import { commands, type ChatMove, type MoveTold, type OpenChat, type PlaneId } from "./bindings";
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
 * **Asks for chat `session` to move to pool `to`, or here for `null`**: the core checks it now
 * and prepares it. Its refusal is held as a failed move, which the pane says.
 */
export async function askMove(plane: PlaneId, session: number, to: string | null) {
  const said = await commands
    .moveChat(plane, session, to)
    .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
  if (said.status === "error") heardMove(plane, { session, to, stage: "failed", why: said.error });
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

/** What chat `session`'s pane says of its move, and whether it is trouble. */
export function moveSaid(one: ChatMove): { words: string; trouble: boolean } {
  const where = placeSaid(one.to);
  switch (one.stage) {
    case "preparing":
      return {
        words: `Moving to ${where}. It keeps working here while ${one.to === null ? "its work comes back" : "the runner is readied"}, and moves at its next pause.`,
        trouble: false,
      };
    case "ready":
      return { words: `Moves to ${where} at its next pause.`, trouble: false };
    default:
      return {
        words: `Did not move to ${where}, so it runs where it was. ${one.why ?? ""}`.trim(),
        trouble: true,
      };
  }
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
