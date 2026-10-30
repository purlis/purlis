// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { atAPause, stateOf, type ChatStates } from "./chatState";

/**
 * **When a chat owed a restart restarts** (#1986, ruling Q63): at its next pause between turns,
 * never in the middle of one, so a build it is running is not cut off by a grant.
 *
 * A pause is what the chat's harness already tells purlis: its turn ended or it handed control
 * back (`waiting`, from its Stop and Notification hooks). Nothing new is read to learn it. While
 * it is mid-turn the restart waits; a harness that says nothing of its turns is never restarted
 * behind the person's back. **A chat waiting on its own work** (#3878, #3887) is not at a pause
 * ({@link atAPause}): its monitors and background shells run, and a restart would cut them off,
 * so the restart waits for that work to wake it and its turn to end with nothing in flight. A
 * chat that reads as ended (`done`, `failed`) is restarted by nothing: what was allowed reaches
 * it at its next start, and once its program has ended it is owed nothing
 * ({@link owedAfterEnded}).
 *
 * **Restart now** is the person's own: it restarts the chat at once, mid-turn or not, unless
 * its program has ended.
 *
 * Every grant before the pause makes one restart, which takes them all: the owed list holds a
 * chat once. A restart the core says is not yet possible (a permission prompt is open) stays
 * owed, and is asked for again at the chat's next move.
 */
export function restartsDue(
  owed: readonly number[],
  states: ChatStates,
  pressed?: { now: number },
): { restart: number[]; ended: number[] } {
  const restart: number[] = [];
  const ended: number[] = [];
  for (const session of owed) {
    const state = stateOf(states, session);
    if (state === "done" || state === "failed") ended.push(session);
    else if (atAPause(states, session) || pressed?.now === session) restart.push(session);
  }
  return { restart, ended };
}

/** `owed` with chat `session` owed its restart: once, however many grants owe it. */
export function withRestartOwed(owed: readonly number[], session: number): readonly number[] {
  return owed.includes(session) ? owed : [...owed, session];
}

/**
 * **What is owed once chats in `ended` read as ended** (#1986): a chat's hooks can say its
 * session ended while its program still runs, so the window drops a restart only where the core,
 * which hears the program itself end (`its_program_ended`), owes it no more (`coreOwes`, read
 * again from `owed_restarts`). Every other chat keeps what it is owed.
 */
export function owedAfterEnded(
  owed: readonly number[],
  ended: readonly number[],
  coreOwes: readonly number[],
): readonly number[] {
  return owed.filter((session) => !ended.includes(session) || coreOwes.includes(session));
}
