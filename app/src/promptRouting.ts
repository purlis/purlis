// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef } from "react";
import type { ChatPromptRouting, PlaneId } from "./bindings";
import { listen } from "./here";

/**
 * **A chat's latest prompt changed whether it fell back** (#4445): the core keeps that routing in
 * memory, so `plane-changed`, which is about disk, never carries it. Calls `heard` with the chat's
 * session for each such telling of `plane`, for whoever draws it to read it again: the telling
 * says only which chat, so tellings landing out of order leave nothing to get wrong.
 *
 * Listens only while `on`. A window that cannot listen reads at its other changes, as before.
 */
export function usePromptRoutingMoved(
  plane: PlaneId,
  on: boolean,
  heard: (session: number) => void,
) {
  const latest = useRef(heard);
  useEffect(() => {
    latest.current = heard;
  });
  useEffect(() => {
    if (!on) return;
    let gone = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await listen<ChatPromptRouting>("chat-prompt-routing", (event) => {
          if (!gone && event.payload.plane === plane) latest.current(event.payload.session);
        });
        if (gone) await unlisten();
        else stop = unlisten;
      } catch {
        // No window to listen in.
      }
    })();
    return () => {
      gone = true;
      stop?.();
    };
  }, [plane, on]);
}
