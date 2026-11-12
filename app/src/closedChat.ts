// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A closed or finished chat, found by its number** (#6214, parent #5820): what the window
 * says of one, and the palette's row that opens its card. The lookup is the core's
 * (`chatcard.rs`, `purlis_core::chatlookup`), one command a pane's chat links (#6217) call too.
 *
 * - **Its card** (`ClosedChatCard.tsx`) says "#641 · <title>", how and when it ended, who
 *   started it, where it ran and its report, with *Read it* (its kept last screen, read-only, in
 *   a view tab) and *Reopen* (its conversation started again as an ordinary chat).
 * - **The palette's finished row** opens it: typing `641` or `#641` finds it as the chat's own
 *   rows are found (#6213, `chatNumberIn`), beside the chat itself where it is open again under
 *   another number.
 */
import { useEffect, useRef, useState } from "react";
import {
  commands,
  type ChatCard,
  type ChatLookup,
  type FinishedTask,
  type PlaneId,
} from "./bindings";
import { qualifierOf, shownOf } from "./finished";
import { endedAt } from "./chatEnded";
import { chatNumberIn, chatNumberSaid, numberedName } from "./chatNumber";
import type { ViewRef } from "./tabs";
import type { Offer } from "./actions";

/** The badge on a card, and the words that say how it ended. */
export type CardOutcome = { badge: string; variant: "ok" | "waiting" | "neutral"; said: string };

/**
 * **How a card says its task ended**: the words a finished row says (`finished.ts`, `shownOf`
 * and `qualifierOf`, one map for the window), capitalised for a badge and a sentence. A badge is
 * the success colour for a task done, the waiting colour for one that failed or was stopped, and
 * plain otherwise.
 */
export function outcomeOf(task: Pick<FinishedTask, "how" | "outcome">): CardOutcome {
  const shown = shownOf(task);
  const word = shown?.word ?? task.outcome;
  const more = qualifierOf(task);
  const variant =
    shown?.kind === "done"
      ? "ok"
      : shown?.token === "state.failed" || shown?.shape === "square"
        ? "waiting"
        : "neutral";
  return {
    badge: capitalised(word),
    variant,
    said: capitalised(more === undefined ? word : `${word} (${more})`),
  };
}

function capitalised(words: string): string {
  return words.charAt(0).toUpperCase() + words.slice(1);
}

export { endedAt } from "./chatEnded";

/** The card's title: `#641 · Signed commits #5607`. */
export function cardTitle(card: ChatCard): string {
  return numberedName(card.number, card.task.name);
}

/** The card's line under its title: "Done at 11:08 · its chat has closed". */
export function cardWhen(card: ChatCard): string {
  const at = endedAt(card.task.ended);
  const how = outcomeOf(card.task).said;
  return `${at === undefined ? how : `${how} at ${at}`} · its chat has closed`;
}

/** What *Read it* says where nothing was kept: the person's ruling of 2026-11-11. */
export const SCREEN_NOT_KEPT = "Its screen was not kept.";

/** What a card says of a task that sent no report. */
export const NO_REPORT = "It sent no report.";

/** The view *Read it* opens: the kept screen of chat `chat` (its ULID), read-only. */
export function chatScreenView(card: ChatCard): ViewRef {
  return { from: null, view: CHAT_SCREEN, key: card.chat };
}

/** The view's name: a builtin view, keyed by the chat's ULID. */
export const CHAT_SCREEN = "chat-screen";

/** The tab *Read it* opens is called by the card's title, and says it is read-only. */
export function chatScreenTitle(card: ChatCard): string {
  return `${cardTitle(card)} (read-only)`;
}

/** The palette's finished row's id, by the number it was found by. */
export function cardRowId(number: number): string {
  return `chat.card:${number}`;
}

/** The palette's row for a closed chat: "Show closed chat #641 · Signed commits #5607". */
export function cardRowTitle(card: ChatCard): string {
  return `Show closed chat ${cardTitle(card)}`;
}

/** The palette's row for a chat open again under `now`, found by its old number `was`. */
export function restartedRowTitle(was: number, now: number): string {
  return `Show chat ${chatNumberSaid(now)}, started again from ${chatNumberSaid(was)}`;
}

/**
 * **What the number the palette's query asks for names** (#6214), as the core answers it: the
 * newest answer to the query typed now, none while the query is no number or there is no project
 * to ask. An answer to an older query is never shown, and a refusal names nothing.
 */
export function useChatByNumber(plane: PlaneId | undefined, query: string): ByNumber | undefined {
  const number = chatNumberIn(query);
  const [answered, setAnswered] = useState<{ asked: string; lookup: ChatLookup }>();
  const newest = useRef(0);
  const asked = `${plane ?? ""}\n${number ?? ""}`;
  useEffect(() => {
    if (plane === undefined || number === undefined) return;
    const ask = ++newest.current;
    void commands
      .chatByNumber(plane, number)
      .then((said) => {
        if (ask !== newest.current) return;
        // Only a lookup is an answer: anything else names nothing.
        const data: unknown = said.status === "ok" ? said.data : undefined;
        const lookup =
          typeof data === "object" && data !== null && "is" in data
            ? (data as ChatLookup)
            : NO_CHAT;
        setAnswered({ asked, lookup });
      })
      .catch(() => {
        if (ask === newest.current) setAnswered({ asked, lookup: NO_CHAT });
      });
  }, [plane, number, asked]);
  if (plane === undefined || number === undefined || answered?.asked !== asked) return undefined;
  return { number, lookup: answered.lookup };
}

const NO_CHAT: ChatLookup = { is: "no_chat" };

/** What a number the palette was asked for names: the number, and the core's lookup of it. */
export type ByNumber = { number: number; lookup: ChatLookup };

/** Whether palette row `offer` is a finished chat's, which opens its card in place. */
export function opensACard(offer: Offer): boolean {
  return offer.id.startsWith("chat.card:");
}

/**
 * **The palette's rows for what a number names** (#6214), beside the rows #6213 finds an open
 * chat by: a closed or finished chat's row, which opens its card, and a chat open again under
 * another number, which shows it. None for an open chat (its own rows are found already) and
 * for no chat. Each carries the number, so the search ranks it with the chat's own rows.
 */
export function numberRows(byNumber: ByNumber | undefined): Offer[] {
  if (byNumber === undefined) return [];
  const { number, lookup } = byNumber;
  if (lookup.is === "ended")
    return [
      {
        id: cardRowId(number),
        title: cardRowTitle(lookup.card),
        available: true,
        reason: "",
        does: { verb: "nothing" },
        name: lookup.card.task.name,
        note: `${cardWhen(lookup.card)}. Shows its card, with Read it and Reopen.`,
        chat: number,
      },
    ];
  if (lookup.is === "restarted")
    return [
      {
        id: `chat.restarted:${number}`,
        title: restartedRowTitle(number, lookup.now),
        available: true,
        reason: "",
        does: { verb: "showChat", session: lookup.now },
        chat: number,
      },
    ];
  return [];
}
