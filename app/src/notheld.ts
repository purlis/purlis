// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type ProjectNotHeld } from "./bindings";
import { listen } from "./here";

/** The event the core sends each time the projects not held change. */
export const NOT_HELD_CHANGED = "projects-not-held";

/**
 * **The projects a runner would not hold again after a re-link** (#2846): what the core keeps
 * from the runner's refusal until a hold of the project is taken. Read as the Pools group comes
 * on screen, and read again each time the core says they changed.
 */
export function useProjectsNotHeld(): readonly ProjectNotHeld[] {
  const [now, setNow] = useState<readonly ProjectNotHeld[]>([]);
  useEffect(() => {
    let gone = false;
    let stop: (() => void) | undefined;
    const read = async () => {
      try {
        const said = await commands.projectsNotHeld();
        if (!gone && said.status === "ok" && Array.isArray(said.data)) setNow(said.data);
      } catch {
        // What cannot be read shows nothing.
      }
    };
    void (async () => {
      try {
        const unlisten = await listen(NOT_HELD_CHANGED, () => {
          void read();
        });
        if (gone) unlisten();
        else stop = unlisten;
      } catch {
        // No window to listen in: a unit test, or a webview being torn down.
      }
      await read();
    })();
    return () => {
      gone = true;
      stop?.();
    };
  }, []);
  return now;
}
