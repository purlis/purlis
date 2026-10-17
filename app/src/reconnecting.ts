// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type ChatsReconnecting, type PlaneId } from "./bindings";
import { listen } from "./here";

/** The event the core sends when a runner's chats start or stop reconnecting. */
export const RECONNECTING = "chats-reconnecting";

/** What a pane says over a chat's last screen while it is reconnecting. */
export const RECONNECTING_SAID = "Reconnecting…";

/**
 * **Whether chat `session` is reconnecting to the runner it runs on** (#2496, ADR 0078 as
 * amended 2026-10-18).
 *
 * This machine's session host keeps the link to a runner and links again on its own when it
 * drops; the core tells the window which chats are reconnecting and which are reconnected
 * (`chats-reconnecting`). A pane reads where its chat stands as it mounts, so one drawn during
 * an outage starts dimmed, and follows what the core says from then on.
 */
export function useReconnecting(plane: PlaneId, session: number): boolean {
  // Kept with the chat it was said of, so a pane drawing another chat starts from nothing.
  const [said, setSaid] = useState<{ plane: PlaneId; session: number } | null>(null);
  useEffect(() => {
    let gone = false;
    let stop: (() => void) | undefined;
    // A word heard before the read answered is newer than the read, and wins.
    let heard = false;
    const show = (reconnecting: boolean) => setSaid(reconnecting ? { plane, session } : null);
    void (async () => {
      try {
        const unlisten = await listen<ChatsReconnecting>(RECONNECTING, (event) => {
          const told = event.payload;
          if (told.plane !== plane || !told.sessions.includes(session)) return;
          heard = true;
          show(told.reconnection === "reconnecting");
        });
        if (gone) unlisten();
        else stop = unlisten;
      } catch {
        // No window to listen in: a unit test, or a webview being torn down.
      }
      try {
        const now = await commands.chatsReconnecting(plane);
        if (!gone && !heard && now.status === "ok") show(now.data.includes(session));
      } catch {
        // What cannot be read shows nothing.
      }
    })();
    return () => {
      gone = true;
      stop?.();
    };
  }, [plane, session]);
  return said?.plane === plane && said.session === session;
}
