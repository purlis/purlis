// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";

import { CLOSE_HANDED, type CloseHanded, type PlaneId } from "./bindings";

/** A link's close handed to this window (#2761): the project, the chats, and a count of asks. */
export type CloseAsk = { plane: PlaneId; sessions: readonly number[]; at: number };

/**
 * **A link's close of chats with theme drafts, as this window hears it** (#2761): the core tells
 * the window holding the project ([`CLOSE_HANDED`]), the project comes to the front
 * (`bringForward`), and its view asks the person, once per ask. A project this window does not
 * hold (`holds`) is not asked about.
 */
export function useCloseHanded(
  holds: (plane: PlaneId) => boolean,
  bringForward: (plane: PlaneId) => void,
): CloseAsk | undefined {
  const [asked, setAsked] = useState<CloseAsk>();
  const latest = useRef({ holds, bringForward });
  useEffect(() => {
    latest.current = { holds, bringForward };
  });
  useEffect(() => {
    const listening = listen<CloseHanded>(CLOSE_HANDED, (event) => {
      const { plane, sessions } = event.payload;
      if (!latest.current.holds(plane)) return;
      latest.current.bringForward(plane);
      setAsked((was) => ({ plane, sessions, at: (was?.at ?? 0) + 1 }));
    }).catch(() => undefined);
    return () => void listening.then((stop) => stop?.()).catch(() => undefined);
  }, []);
  return asked;
}
