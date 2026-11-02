// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **One state per task question** (#4564, spec #4551, ADR 0107): with its asking chat, or
 * waiting on the person. Derived here from the asks registry (`asks.ts`) and nowhere else, so
 * the task's row, its tab's hand, the ✋ and the Inbox read the same thing and none keeps a
 * copy of its own.
 *
 * | Level | Where it shows |
 * |---|---|
 * | waiting on you | the ✋ count, the hand on the tab and the rows above, the Inbox, the row |
 * | with the asking chat | the task's row only, and `n asking` on a folded row above it |
 *
 * Plain inputs and no state: nothing here reads a chat, a store or the core.
 */
import type { Shown, WithAskingChat } from "./bindings";
import { rowTimeSaid } from "./chatsList";

/** A project's asks as the registry derived them: what waits on the person, and the task
 *  questions still with their asking chat. */
export type QuestionsHeld = {
  readonly asks: readonly Shown[];
  readonly with_asking_chat: readonly WithAskingChat[];
};

/** Where a task's question stands. `since` is in seconds since 1970: when it was asked, or
 *  when it reached the person. */
export type QuestionStanding =
  | { level: "with-asker"; asker: string; since: number }
  | { level: "needs-you"; since: number | null };

/** Task `session`'s open question, or nothing where it has none. */
export function standingOf(
  held: QuestionsHeld | undefined,
  session: number,
): QuestionStanding | undefined {
  if (held === undefined) return undefined;
  const reached = held.asks.find(
    (ask) => ask.source === "task-question" && ask.session === session,
  );
  if (reached !== undefined) return { level: "needs-you", since: reached.since ?? null };
  const asking = held.with_asking_chat.find((one) => one.session === session);
  return asking === undefined
    ? undefined
    : { level: "with-asker", asker: asking.asker, since: asking.since };
}

/** The tasks whose question waits on the person, oldest first: never one with its asking
 *  chat, which adds nothing to a count or a hand. */
export function waitingOnYou(held: QuestionsHeld | undefined): number[] {
  return (held?.asks ?? [])
    .filter((ask) => ask.source === "task-question")
    .map((ask) => ask.session);
}

/** What a task's row says of its question: who it waits on, and for how long, in the row's
 *  own time words (`now`, then whole minutes), from `since`. */
export function questionWord(standing: QuestionStanding, now: Date): string {
  const { who, since } = questionWords(standing, now);
  return since === null ? who : `${who} · ${since}`;
}

/** The same words in their two parts, so a row can let the first give way and keep the time
 *  whole: who it waits on, and how long (`null` where it is not known). */
export function questionWords(
  standing: QuestionStanding,
  now: Date,
): { who: string; since: string | null } {
  const who = standing.level === "needs-you" ? "needs you" : `asking ${standing.asker}`;
  if (standing.since === null) return { who, since: null };
  const seconds = Math.max(0, Math.floor(now.getTime() / 1000) - standing.since);
  return { who, since: rowTimeSaid(seconds, now) };
}

/** What a folded row says of the questions under it that are with their asking chat: never
 *  red, and never counted on the ✋ (the person's ruling on #4551). */
export function askingSaid(count: number): string {
  return `${count} asking`;
}

/**
 * **How many task questions are with their asking chat under each row**, at any depth, by the
 * row's session. Rows are in the list's order, each below the row before it with a greater
 * level. A row with none under it is not in the map.
 */
export function askingBelow(
  rows: readonly { session: number; level: number }[],
  held: QuestionsHeld | undefined,
): ReadonlyMap<number, number> {
  const counts = new Map<number, number>();
  const asking = new Set((held?.with_asking_chat ?? []).map((one) => one.session));
  if (asking.size === 0) return counts;
  rows.forEach((row, at) => {
    let count = 0;
    for (let next = at + 1; next < rows.length && rows[next].level > row.level; next += 1) {
      if (asking.has(rows[next].session)) count += 1;
    }
    if (count > 0) counts.set(row.session, count);
  });
  return counts;
}
