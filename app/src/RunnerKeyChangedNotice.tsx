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
 * dimmed, and this says why, once for the project, with **Open Pools settings**, where its pool is
 * removed, and **Dismiss**. It goes once the core says the chats are reconnected.
 *
 * **A cluster runner's is the same Notice** (#3292): its host refuses a link before it is made
 * when the cluster's Pool names another cluster operator key than the one pinned, or the operator
 * no longer vouches for the pod and key the runner was paired with, and says so as `key_changed`.
 */
export function RunnerKeyChangedNotice({
  plane,
  onOpenPools,
}: {
  plane: PlaneId;
  /** Opens Settings at this machine's Pools. */
  onOpenPools: () => void;
}) {
  const { shown, dismiss } = useKeyChanged(plane);
  if (!shown) return null;
  return (
    <Notice
      cause={`runner-key-changed:${plane}`}
      tone="trouble"
      label="Runner key changed"
      link={{ label: "Open Pools settings", onPress: onOpenPools }}
      onDismiss={dismiss}
    >
      The runner this project&apos;s chats run on answered with another key than the one it was
      paired with, or its cluster no longer vouches for that key, so purlis sends it nothing.
      Remove its pool and add it again to run them there.
    </Notice>
  );
}

/** What the core last said of `plane`'s runner's key, kept with the plane it was said of. */
type Said = { plane: PlaneId; changed: boolean; dismissed: boolean };

/**
 * Whether the Notice is shown for `plane`: the core last said its runner answered with another
 * key, and the person has not put that away. A key said to have changed again shows it again.
 */
function useKeyChanged(plane: PlaneId): { shown: boolean; dismiss: () => void } {
  const [said, setSaid] = useState<Said | null>(null);
  useEffect(() => {
    let gone = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await listen<ChatsReconnecting>(RECONNECTING, (event) => {
          const told = event.payload;
          if (told.plane !== plane) return;
          if (told.reconnection === "key_changed")
            setSaid({ plane, changed: true, dismissed: false });
          else if (told.reconnection === "reconnected")
            setSaid({ plane, changed: false, dismissed: false });
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
  const shown = said !== null && said.plane === plane && said.changed && !said.dismissed;
  const dismiss = () => setSaid((was) => (was === null ? was : { ...was, dismissed: true }));
  return { shown, dismiss };
}
