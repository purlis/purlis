// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import { useChatsHere, useChatsSelect } from "./chatState";

/**
 * **How many times a chat has started in this project** since the caller mounted: a dependency
 * for a read that should be fresh when a chat starts (#1407, #1405), such as the hosts Notices,
 * whose state another window or a teammate's pull may have changed since they last read it.
 *
 * A start is a chat the project's chats are first heard from (`ChatStates.bySession` gains its
 * number), as `chat-moved` tells it; nothing is polled. The chats already there when the window
 * first hears of them count as one start, so the read they cause is at most one more than the
 * mount's. A chat that closes is not a start. Starts at 0, and with no project above it (a test
 * or a preview drawing the component alone) never moves.
 */
export function useChatStarts(): number {
  const chats = useChatsHere();
  // One string per set of chats, so a move that changes only a chat's state is the same answer
  // and redraws nothing.
  const sessions = useChatsSelect(chats, (states) => Object.keys(states.bySession).join(" "));
  const [starts, setStarts] = useState(0);
  const heard = useRef<ReadonlySet<string>>(new Set());
  useEffect(() => {
    const now = new Set(sessions === "" ? [] : sessions.split(" "));
    const fresh = [...now].some((session) => !heard.current.has(session));
    heard.current = now;
    if (fresh) setStarts((n) => n + 1);
  }, [sessions]);
  return starts;
}
