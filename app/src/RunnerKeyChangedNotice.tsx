// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import type { ChatsReconnecting, PlaneId } from "./bindings";
import { listen } from "./here";
import { Notice } from "./Notice";
import { RECONNECTING } from "./reconnecting";

/**
 * **The runner a project's chats run on answered with another key** (#2752, ADR 0078 as amended
 * 2026-10-20): the Notice the window shows, at its own level and not on each chat's pane.
 *
 * This machine's session host relays the window's chats on a runner over a link it keeps. When
 * the runner answers that link with another key than the one pinned at its pairing, the host
 * sends it nothing and does not link to it again until the person removes the runner and adds
 * it anew; the core says so as `key_changed` (`chats-reconnecting`). Each chat's pane stays
 * dimmed, and this says why, once for the project, with **Open Pools**, where its pool is
 * removed, and **Dismiss**. It goes once the core says the chats are reconnected.
 */
export function RunnerKeyChangedNotice({
  plane,
  onOpenPools,
}: {
  plane: PlaneId;
  /** Opens Settings at this machine's Pools. */
  onOpenPools: () => void;
}) {
  const changed = useKeyChanged(plane);
  const [dismissed, setDismissed] = useState(false);
  useEffect(() => {
    if (!changed) setDismissed(false);
  }, [changed]);
  if (!changed || dismissed) return null;
  return (
    <Notice
      cause={`runner-key-changed:${plane}`}
      tone="trouble"
      label="Runner key changed"
      link={{ label: "Open Pools", onPress: onOpenPools }}
      onDismiss={() => setDismissed(true)}
    >
      The runner this project&apos;s chats run on answered with another key than the one it was
      paired with, so purlis sends it nothing. Remove its pool and add it again to run them there.
    </Notice>
  );
}

/** Whether the core last said `plane`'s runner answered with another key. */
function useKeyChanged(plane: PlaneId): boolean {
  const [changed, setChanged] = useState(false);
  useEffect(() => {
    setChanged(false);
    let gone = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await listen<ChatsReconnecting>(RECONNECTING, (event) => {
          const told = event.payload;
          if (told.plane !== plane) return;
          if (told.reconnection === "key_changed") setChanged(true);
          else if (told.reconnection === "reconnected") setChanged(false);
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
  }, [plane]);
  return changed;
}
