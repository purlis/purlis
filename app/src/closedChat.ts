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
import { commands, type ChatCard, type ChatLookup, type How, type PlaneId } from "./bindings";
import { chatNumberIn, chatNumberSaid, numberedName } from "./chatNumber";
import type { ViewRef } from "./tabs";
import type { Offer } from "./actions";

/** The badge on a card, and the words that say how it ended: Finished, Blocked, Failed, or
 *  Closed where the person or purlis ended it (D8 of the prototype's map). */
export type CardOutcome = { badge: string; variant: "ok" | "waiting" | "neutral"; said: string };

/** How a card says each end. Only a task the person closed reads "Closed"; one purlis stopped
 *  at a limit says so in its words. */
export function outcomeOf(how: How): CardOutcome {
  switch (how) {
    case "done":
      return { badge: "Finished", variant: "ok", said: "Finished" };
    case "cancelled":
      return { badge: "Cancelled", variant: "neutral", said: "Cancelled by the chat that asked" };
    case "blocked":
      return { badge: "Blocked", variant: "waiting", said: "Stopped blocked" };
    case "failed":
      return { badge: "Failed", variant: "waiting", said: "Failed" };
    case "unreported":
      return { badge: "Failed", variant: "waiting", said: "Ended without a report" };
    case "last_message":
      return { badge: "Finished", variant: "neutral", said: "Ended on its last message" };
    case "stopped_by_person":
      return { badge: "Closed", variant: "neutral", said: "Stopped by you" };
    case "closed_by_person":
      return { badge: "Closed", variant: "neutral", said: "Closed by you" };
    case "stopped_at_limit":
      return { badge: "Stopped", variant: "waiting", said: "Stopped at its time limit" };
    case "stopped_with_above":
      return { badge: "Stopped", variant: "waiting", said: "Stopped with the task above it" };
    case "stopped_at_token_limit":
      return { badge: "Stopped", variant: "waiting", said: "Stopped at its token limit" };
  }
}

/** When it ended, as a clock says it: `11:08`. Nothing where the record keeps no end. */
export function endedAt(ended: string | null): string | undefined {
  if (ended === null) return undefined;
  const at = new Date(ended);
  if (Number.isNaN(at.getTime())) return undefined;
  return at.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false });
}

/** The card's title: `#641 · Signed commits #5607`. */
export function cardTitle(card: ChatCard): string {
  return numberedName(card.number, card.task.name);
}

/** The card's line under its title: "Finished at 11:08 · it is in no list of open chats now". */
export function cardWhen(card: ChatCard): string {
  const at = endedAt(card.task.ended);
  const how = outcomeOf(card.task.how).said;
  return `${at === undefined ? how : `${how} at ${at}`} · it is in no list of open chats now`;
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

/** The palette's finished row's title: "Finished chat #641 · Signed commits #5607". */
export function cardRowTitle(card: ChatCard): string {
  return `${outcomeOf(card.task.how).badge} chat ${cardTitle(card)}`;
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
export function useChatByNumber(
  plane: PlaneId | undefined,
  query: string,
): { number: number; found: ChatLookup } | undefined {
  const number = chatNumberIn(query);
  const [found, setFound] = useState<{ asked: string; found: ChatLookup }>();
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
        const answer =
          typeof data === "object" && data !== null && "found" in data
            ? (data as ChatLookup)
            : ({ found: "no_chat" } as const);
        setFound({ asked, found: answer });
      })
      .catch(() => {
        if (ask === newest.current) setFound({ asked, found: { found: "no_chat" } });
      });
  }, [plane, number, asked]);
  if (plane === undefined || number === undefined || found?.asked !== asked) return undefined;
  return { number, found: found.found };
}

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
export function numberRows(byNumber: { number: number; found: ChatLookup } | undefined): Offer[] {
  if (byNumber === undefined) return [];
  const { number, found } = byNumber;
  if (found.found === "ended")
    return [
      {
        id: cardRowId(number),
        title: cardRowTitle(found.card),
        available: true,
        reason: "",
        does: { verb: "nothing" },
        name: found.card.task.name,
        note: "Shows its card: how it ended, its report, Read it and Reopen.",
        chat: number,
      },
    ];
  if (found.found === "restarted")
    return [
      {
        id: `chat.restarted:${number}`,
        title: restartedRowTitle(number, found.now),
        available: true,
        reason: "",
        does: { verb: "showChat", session: found.now },
        chat: number,
      },
    ];
  return [];
}
