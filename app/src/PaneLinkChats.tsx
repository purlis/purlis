// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import type { ChatCard, ChatGo } from "./bindings";
import { readItOffer, reopenTaskOffer, showLinkedChatOffer, type Offer } from "./actions";
import { ClosedChatCardPopover } from "./ClosedChatCard";
import { placed } from "./PaneLinkSurfaces";
import type { LinkPlace } from "./paneLinks";

/**
 * **What a pane's `chat 641` link shows when ⌘-clicked** (#6217): an open chat in its tab or
 * workspace, an ended chat's card beside the link (the prototype's D7, `ClosedChatCard.tsx`), or nothing for the
 * pane's own chat (the ruling on #6211's Q3).
 *
 * - **It runs as the pane's rows run** (`onRun`, the pane's menu's `onPress`): the chat is
 *   shown by the window's `showChat`, and the card's *Read it* and *Reopen* by the offers the
 *   palette's card runs (`readItOffer`, `reopenTaskOffer`). Nothing here starts a program;
 *   *Reopen* does only on the person's press.
 * - **Apart from the link surfaces** (`PaneLinkSurfaces.tsx`), which a screen that must reach
 *   no core draws too: this reaches the window's actions, and only a pane that has them uses it.
 */
export function usePaneLinkChats(onRun: ((offer: Offer) => void) | undefined): {
  onChat: (go: ChatGo, place: LinkPlace | undefined) => void;
  surfaces: ReactNode;
} {
  const run = useRef(onRun);
  useEffect(() => {
    run.current = onRun;
  });
  const [carded, setCarded] = useState<{ card: ChatCard; place: LinkPlace | undefined } | null>(
    null,
  );
  const onChat = useCallback((go: ChatGo, place: LinkPlace | undefined) => {
    // The pane's own chat is in front already: nothing more.
    if (go.to === "chat") run.current?.(showLinkedChatOffer(go.session));
    else if (go.to === "card") setCarded({ card: go.card, place });
  }, []);
  const anchor = useRef<HTMLSpanElement>(null);
  const press = (offer: Offer) => {
    setCarded(null);
    run.current?.(offer);
  };
  const surfaces = carded !== null && (
    <>
      <span
        ref={anchor}
        className="pane-link-anchor"
        aria-hidden="true"
        style={placed(carded.place)}
      />
      <ClosedChatCardPopover
        card={carded.card}
        triggerRef={anchor}
        isOpen
        onOpenChange={(isOpen) => {
          if (!isOpen) setCarded(null);
        }}
        onRead={() => press(readItOffer(carded.card))}
        onReopen={() => press(reopenTaskOffer(carded.card))}
      />
    </>
  );
  return { onChat, surfaces };
}
