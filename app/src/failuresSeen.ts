// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from "react";
import { commands, type PlaneId } from "./bindings";
import { failuresSeenBy, type ChatStates } from "./chatState";

/**
 * **What failed has been seen once its chat is on screen and focused** (#5340): `session` is the
 * chat of the focused pane of the tab in front, and the core is told of each failure looking at
 * it sees (`failuresSeenBy`), as soon as the window has the keyboard. Its mark then goes back to
 * the chat's own state.
 *
 * **The core's own record says "until seen"** (`ChatStates.failedTasks`): the window stores
 * nothing, and its item in the Inbox goes with the look, as it does for every other look. A
 * window in the background sees nothing, nor does a project drawn behind another.
 */
export function useFailuresSeen(
  plane: PlaneId,
  session: number | undefined,
  failedTasks: ChatStates["failedTasks"],
  /** Whether this project is the one in front. Handed in: the view that calls this draws the
   *  provider of `useProjectShown`, so that hook would answer for the project above it. */
  shown: boolean,
): void {
  useEffect(() => {
    if (!shown || session === undefined) return;
    const seen = failuresSeenBy({ failedTasks }, session);
    if (seen.length === 0) return;
    const told = () => {
      if (!document.hasFocus()) return;
      for (const { asker, id } of seen)
        void commands.taskFailureSeen(plane, asker, id).catch(() => undefined);
    };
    told();
    window.addEventListener("focus", told);
    return () => window.removeEventListener("focus", told);
  }, [plane, session, failedTasks, shown]);
}
