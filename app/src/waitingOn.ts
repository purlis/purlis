// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What the waiting strip shows** (#4563, spec #4551 stories 40 to 48): what a chat waiting on
 * the person is waiting on, read off the asks registry, the Inbox's own list, so the strip and
 * the Inbox card say the same words and one answer clears both.
 *
 * - **A real ask** (a permission, or Claude Code's question tool on its permission hook) is
 *   shown as asked: its own words, held with the ask, which no later turn replaces.
 * - **A prompt in its harness's terminal** that purlis holds nothing for is shown by its kind.
 * - **Prose**, a chat whose turn ended on the person, shows the message it ended on, and the few
 *   before it can be read back (`Ledger::last_said`, `Ledger::said_before`).
 *
 * Every other ask has a Notice of its own in the band below the strip, which says it.
 */
import type { Shown } from "./bindings";

/** Every word the strip says (the prototype's `copy.ts`, as the person reviewed it). */
export const STRIP = {
  /** The strip's name, and its label over a real ask or a prompt. */
  waitingOn: "Waiting on you",
  showAll: "Show all",
  showLess: "Show less",
  lastMessage: "Its last message",
  earlier: (n: number) => (n === 1 ? "1 earlier message" : `${n} earlier messages`),
  earlierAt: (at: number, of: number) => `Message ${at} of ${of} before the last`,
  previous: "Earlier message",
  next: "Later message",
  backToLast: "Back to the last",
  /** Where a permission or a prompt is answered on a chat on screen: its harness's own prompt. */
  answerBelow: "Answer it in the terminal below",
  /** The app heard no message from it: its harness gave none, or purlis started since. */
  noMessage: "No message to show. Scroll up in the terminal below to read what it said.",
} as const;

/** What a chat waits on the person for, as the strip draws it. */
export type Wait =
  | { kind: "ask"; ask: string; says: string }
  | { kind: "prompt"; ask: string; says: string }
  | { kind: "prose"; ask: string; says: string; earlier: readonly string[] };

/** The order the strip takes a chat's asks in: what stops its turn now before its turn's end. */
const FIRST: readonly Shown["source"][] = ["permission", "terminal", "question"];

/**
 * **What chat `session` waits on the person for**, from `waiting`, the registry's list, or
 * nothing while it waits on them for none of the three. One at a time: a permission or a prompt
 * stops the turn the chat is in, so it comes before what an earlier turn ended on.
 */
export function waitOf(waiting: readonly Shown[] | undefined, session: number): Wait | undefined {
  const its = (waiting ?? []).filter((ask) => ask.session === session);
  for (const source of FIRST) {
    const ask = its.find((one) => one.source === source);
    if (ask === undefined) continue;
    if (source === "permission") return { kind: "ask", ask: ask.ask, says: ask.says };
    if (source === "terminal") return { kind: "prompt", ask: ask.ask, says: ask.says };
    return {
      kind: "prose",
      ask: ask.ask,
      says: ask.last_message ?? STRIP.noMessage,
      earlier: ask.earlier_messages ?? [],
    };
  }
  return undefined;
}

/** **Whether a text is cut on one line**, so Show all is offered: longer than the line holds at
 *  a pane's narrowest, or on several lines. */
export function cutOnOneLine(says: string): boolean {
  return says.length > 90 || says.includes("\n");
}
