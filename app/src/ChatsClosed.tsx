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
import { numberedName } from "./chatNumber";
import { TreeRow } from "./ui/tree/Tree";
import { RowLabel, RowTrail } from "./ui/list/Row";

/** The *Closed chats* row's key among the tree's rows. */
export const CLOSED_CHATS = "closed-chats";

const NONE: readonly ClosedChat[] = [];

/**
 * **The closed chats of `plane`, read as the filter starts to ask** (`asking`): none while it
 * does not, and the newest read while it does, read again each time it starts again, so a chat
 * closed meanwhile is found. A refusal finds none.
 */
export function useClosedChats(plane: PlaneId | undefined, asking: boolean): readonly ClosedChat[] {
  const [read, setRead] = useState<readonly ClosedChat[]>(NONE);
  useEffect(() => {
    if (plane === undefined || !asking) return;
    let current = true;
    void commands
      .closedChats(plane)
      .then((said) => {
        // Only a list is an answer: anything else finds none.
        if (current) setRead(said.status === "ok" && Array.isArray(said.data) ? said.data : NONE);
      })
      .catch(() => {
        if (current) setRead(NONE);
      });
    return () => {
      current = false;
    };
  }, [plane, asking]);
  return asking ? read : NONE;
}

/** **The *Closed chats* row**, with a row for each closed chat the filter found, or nothing. */
export function closedChatsRow({
  closed,
  plane,
  onPress,
}: {
  closed: readonly ClosedChat[];
  plane: PlaneId | undefined;
  /** Runs the card's *Read it* and *Reopen*, and shows a chat open again. */
  onPress?: (offer: Offer) => void;
}): ReactNode {
  if (closed.length === 0 || plane === undefined) return null;
  return (
    <TreeRow
      id={CLOSED_CHATS}
      textValue="Closed chats"
      className="chats-closed"
      alwaysOpen
      content={<RowLabel className="session">Closed chats</RowLabel>}
    >
      {closed.map((chat) => (
        <ClosedChatRow key={chat.number} chat={chat} plane={plane} onPress={onPress} />
      ))}
    </TreeRow>
  );
}

/** **One closed chat's row**: chosen, it opens the chat's card beside it. */
function ClosedChatRow({
  chat,
  plane,
  onPress,
}: {
  chat: ClosedChat;
  plane: PlaneId;
  onPress?: (offer: Offer) => void;
}) {
  const row = useRef<HTMLDivElement>(null);
  const [card, setCard] = useState<ChatCard | null>(null);
  const name = numberedName(chat.number, chat.title);
  const at = endedAt(chat.ended);
  const choose = () => {
    void commands
      .chatByNumber(plane, chat.number)
      .then((said) => {
        if (said.status !== "ok") return;
        const lookup = said.data;
        if (lookup.is === "ended") setCard(lookup.card);
        // Open again since the filter read it: that chat, as a link to it shows it.
        else if (lookup.is === "restarted") onPress?.(showLinkedChatOffer(lookup.now));
        else if (lookup.is === "open") onPress?.(showLinkedChatOffer(lookup.number));
      })
      .catch(() => undefined);
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
      className="chat-closed"
      description={at === undefined ? "closed" : `closed at ${at}`}
      onAction={choose}
      content={
        <>
          <RowLabel className="session ui-text-secondary">{name}</RowLabel>
          {at !== undefined && (
            <RowTrail>
              <span className="chat-closed-at ui-text-meta">{at}</span>
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
