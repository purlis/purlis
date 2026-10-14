// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { State } from "./chatState";

/**
 * **When a chat owed a restart restarts** (#1986, ruling Q63): at its next pause between turns,
 * never in the middle of one, so a build it is running is not cut off by a grant.
 *
 * A pause is what the chat's harness already tells purlis: its turn ended or it handed control
 * back (`waiting`, from its Stop and Notification hooks). Nothing new is read to learn it. While
 * it is mid-turn the restart waits; a harness that says nothing of its turns is never restarted
 * behind the person's back. A chat whose program ended first (`done`, `failed`) is restarted
 * by nothing: what was allowed reaches it at its next start, and it is owed nothing now.
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
  stateOf: (session: number) => State,
  pressed?: { now: number },
): { restart: number[]; ended: number[] } {
  const restart: number[] = [];
  const ended: number[] = [];
  for (const session of owed) {
    const state = stateOf(session);
    if (state === "done" || state === "failed") ended.push(session);
    else if (state === "waiting" || pressed?.now === session) restart.push(session);
  }
  return { restart, ended };
}

/** `owed` with chat `session` owed its restart: once, however many grants owe it. */
export function withRestartOwed(owed: readonly number[], session: number): readonly number[] {
  return owed.includes(session) ? owed : [...owed, session];
}
