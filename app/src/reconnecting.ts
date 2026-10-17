// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import type { ChatsReconnecting, PlaneId } from "./bindings";
import { listen } from "./here";

/** The event the core sends when a runner's chats start or stop reconnecting. */
export const RECONNECTING = "chats-reconnecting";

/**
 * **Whether chat `session` is reconnecting to the runner it runs on** (#2496, ADR 0078 as
 * amended 2026-10-17).
 *
 * This machine's session host keeps the link to a runner and links again on its own when it
 * drops; it tells the window when the link is down and when it is back
 * (`chats-reconnecting`), naming the chats that run there. A pane opens its view only while the
 * link is up, so it starts as not reconnecting, and follows what the core says from then on.
 */
export function useReconnecting(plane: PlaneId, session: number): boolean {
  // Kept with the chat it was said of, so a pane drawing another chat starts from nothing.
  const [said, setSaid] = useState<{ plane: PlaneId; session: number } | null>(null);
  useEffect(() => {
    let gone = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await listen<ChatsReconnecting>(RECONNECTING, (event) => {
          const told = event.payload;
          if (told.plane !== plane || !told.sessions.includes(session)) return;
          setSaid(told.reconnecting ? { plane, session } : null);
        });
        if (gone) unlisten();
        else stop = unlisten;
      } catch {
        // No window to listen in: a unit test, or a webview being torn down.
      }
    })();
    return () => {
      gone = true;
      stop?.();
    };
  }, [plane, session]);
  return said?.plane === plane && said.session === session;
}
