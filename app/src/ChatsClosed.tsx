// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Closed chats the Chats list's filter finds** (#6484, leftover from #6452): a chat that
 * closed is found by its number, one it had before a restart, its title or where it ran, and
 * choosing it opens its card (`ClosedChatCard.tsx`), as the palette's row and a pane's
 * `chat 641` link open it.
 *
 * - **Where a closed row sits**: under one *Closed chats* row after every open chat, as the
 *   pinned browsers whose chat closed stand (#2022), always open. Only while words or a number
 *   are typed: a chip asks about a chat still running, as it never finds a finished task.
 * - **How it looks**: a session row's words, `#702 · Tidy the changelog`, in the secondary tier,
 *   with when it closed at its end. No state mark: it runs nothing.
 * - **What it reads**: the app's records of closed chats (`closed_chats`, the core's
 *   `chatlookup::closed_now`), once as the filter starts to ask, and the card's lookup
 *   (`chat_by_number`) as a row is chosen. Nothing here starts a program: *Reopen* on the card
 *   does, on the person's press.
 */
import { useEffect, useRef, useState, type ReactNode } from "react";
import { commands, type ChatCard, type ClosedChat, type PlaneId } from "./bindings";
import { readItOffer, reopenOffer, showLinkedChatOffer, type Offer } from "./actions";
import { ClosedChatCardPopover } from "./ClosedChatCard";
import { endedAt } from "./chatEnded";
import { numberedNameWas } from "./chatNumber";
import { TreeRow } from "./ui/tree/Tree";
import { RowLabel, RowTrail } from "./ui/list/Row";

/** The *Closed chats* row's key among the tree's rows. */
export const CLOSED_CHATS = "closed-chats";

const NONE: readonly ClosedChat[] = [];

/** What the filter read of the closed chats: those it found, and why none were read, if so. */
export type ClosedRead = { chats: readonly ClosedChat[]; failed?: string };

const NOTHING_READ: ClosedRead = { chats: NONE };

/** A read, and the number typed it was asked for, where one was. */
type Answered = { typed: number | null; read: ClosedRead };

/** The words of a refusal or a throw, for a line that says what failed. */
function whyOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * **The closed chats of `plane`, read as the filter starts to ask** (`asking`): none while it
 * does not, and the newest read while it does, read again each time it starts again, each time
 * the number typed (`typed`) changes, and each time the chats open now (`openNow`, their
 * numbers) change while it asks, so a chat that closes meanwhile is found. Asked with a number,
 * the core answers only the chats that had it, each saying whether it was found by an earlier
 * one (#7125); a read for another number than the one typed now is never shown. A refusal finds
 * none, and says why (`failed`).
 */
export function useClosedChats(
  plane: PlaneId | undefined,
  asking: boolean,
  openNow: string,
  typed: number | null,
): ClosedRead {
  const [answered, setAnswered] = useState<Answered>({ typed: null, read: NOTHING_READ });
  useEffect(() => {
    if (plane === undefined || !asking) return;
    let current = true;
    const answer = (read: ClosedRead) => {
      if (current) setAnswered({ typed, read });
    };
    void commands
      .closedChats(plane, typed)
      .then((said) => {
        if (said.status !== "ok") answer({ chats: NONE, failed: whyOf(said.error) });
        // Only a list is an answer: anything else finds none.
        else answer({ chats: Array.isArray(said.data) ? said.data : NONE });
      })
      .catch((error: unknown) => answer({ chats: NONE, failed: whyOf(error) }));
    return () => {
      current = false;
    };
  }, [plane, asking, openNow, typed]);
  return asking && answered.typed === typed ? answered.read : NOTHING_READ;
}

/** The line under the filter when the closed chats could not be read: what, and what to do. */
export function closedNotRead(read: ClosedRead): string {
  return read.failed === undefined
    ? ""
    : `purlis could not read the closed chats (${read.failed}). Clear the filter and type again.`;
}

/** **The *Closed chats* row**, with a row for each closed chat the filter found, or nothing. */
export function closedChatsRow({
  closed,
  typed,
  plane,
  onPress,
  onSay,
}: {
  closed: readonly ClosedChat[];
  /** The number typed in the filter, where one was: what each row's card was found by. */
  typed: number | null;
  plane: PlaneId | undefined;
  /** Runs the card's *Read it* and *Reopen*, and shows a chat open again. */
  onPress?: (offer: Offer) => void;
  /** Says why choosing a row opened nothing. */
  onSay?: (said: string) => void;
}): ReactNode {
  if (closed.length === 0 || plane === undefined) return null;
  return (
    <TreeRow
      id={CLOSED_CHATS}
      textValue="Closed chats"
      alwaysOpen
      content={<RowLabel className="session">Closed chats</RowLabel>}
    >
      {closed.map((chat) => (
        <ClosedChatRow
          key={chat.number}
          chat={chat}
          typed={typed}
          plane={plane}
          onPress={onPress}
          onSay={onSay}
        />
      ))}
    </TreeRow>
  );
}

/** Why choosing closed chat `number` opened no card, and what to do. */
function notOpened(number: number, why: string): string {
  return `purlis could not open chat #${number}'s card (${why}). Choose it again.`;
}

/** **One closed chat's row**: chosen, it opens the chat's card beside it, or says why not. */
function ClosedChatRow({
  chat,
  typed,
  plane,
  onPress,
  onSay,
}: {
  chat: ClosedChat;
  typed: number | null;
  plane: PlaneId;
  onPress?: (offer: Offer) => void;
  onSay?: (said: string) => void;
}) {
  const row = useRef<HTMLDivElement>(null);
  const [card, setCard] = useState<ChatCard | null>(null);
  // Found by a number it had before a restart, it says that one too (#6685), as the core says.
  const name = numberedNameWas(chat.number, chat.title, chat.found_by);
  const at = endedAt(chat.ended);
  const choose = () => {
    void commands
      // By its own ULID, never the number typed, which another record may answer to (#6740).
      // The number typed only labels its card, as the core decides (#7125).
      .chatById(plane, chat.id, typed)
      .then((said) => {
        if (said.status !== "ok") {
          onSay?.(notOpened(chat.number, whyOf(said.error)));
          return;
        }
        const lookup = said.data;
        if (lookup.is === "ended") setCard(lookup.card);
        // Open again since the filter read it: that chat, as a link to it shows it.
        else if (lookup.is === "restarted") onPress?.(showLinkedChatOffer(lookup.now));
        else if (lookup.is === "open") onPress?.(showLinkedChatOffer(lookup.number));
        // Swept since the filter read it.
        else onSay?.(`Chat #${chat.number} is no longer kept, so it has no card to open.`);
      })
      .catch((error: unknown) => onSay?.(notOpened(chat.number, whyOf(error))));
  };
  const press = (offer: Offer) => {
    setCard(null);
    onPress?.(offer);
  };
  return (
    <TreeRow
      ref={row}
      id={`closed:${chat.number}`}
      textValue={name}
      description={at === undefined ? "closed" : `closed at ${at}`}
      onAction={choose}
      content={
        <>
          <RowLabel className="session ui-text-secondary">{name}</RowLabel>
          {at !== undefined && (
            <RowTrail>
              <span className="ui-text-meta">{at}</span>
            </RowTrail>
          )}
          {card !== null && (
            <ClosedChatCardPopover
              card={card}
              triggerRef={row}
              isOpen
              onOpenChange={(isOpen) => {
                if (!isOpen) setCard(null);
              }}
              onRead={() => press(readItOffer(card))}
              onReopen={() => press(reopenOffer(card))}
            />
          )}
        </>
      }
    />
  );
}
