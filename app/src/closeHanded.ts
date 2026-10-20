// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";

import { CLOSE_HANDED, commands, type CloseHanded, type PlaneId } from "./bindings";
import { listen } from "./here";

/**
 * A link's close handed to this window (#2761): the project, the chats, a count of asks, and
 * whether it was dropped, since a question was on screen when it came.
 */
export type CloseAsk = {
  plane: PlaneId;
  sessions: readonly number[];
  at: number;
  dropped: boolean;
};

/**
 * **A link's close of chats with theme drafts, as this window hears it** (#2761): the core tells
 * the window holding the project ([`CLOSE_HANDED`]), and that project's view asks the person,
 * once per ask. A project this window does not hold (`holds`) is not asked about.
 *
 * **Never over a question already on screen** (`asking`, read for the whole window): the hand is
 * dropped, nothing is brought forward and nothing on screen changes; the project's view says it
 * was not asked. Otherwise the project comes to the front (`bringForward`) and the window is
 * raised.
 */
export function useCloseHanded(
  holds: (plane: PlaneId) => boolean,
  asking: () => boolean,
  bringForward: (plane: PlaneId) => void,
): CloseAsk | undefined {
  const [asked, setAsked] = useState<CloseAsk>();
  const latest = useRef({ holds, asking, bringForward });
  useEffect(() => {
    latest.current = { holds, asking, bringForward };
  });
  useEffect(() => {
    const listening = listen<CloseHanded>(CLOSE_HANDED, (event) => {
      const { plane, sessions } = event.payload;
      const now = latest.current;
      if (!now.holds(plane)) return;
      const dropped = now.asking();
      if (!dropped) {
        now.bringForward(plane);
        void commands.showWindowHolding(plane).catch(() => undefined);
      }
      setAsked((was) => ({ plane, sessions, at: (was?.at ?? 0) + 1, dropped }));
    }).catch(() => undefined);
    return () => void listening.then((stop) => stop?.()).catch(() => undefined);
  }, []);
  return asked;
}

/**
 * **Whether a question is on screen anywhere in this window**: a close or a "Who draws it?" that
 * a project's view holds (`held`, every project's), or any other alert dialog drawn.
 */
export function aQuestionIsUp(held: ReadonlySet<PlaneId>): boolean {
  return held.size > 0 || document.querySelector('[role="alertdialog"]') !== null;
}
