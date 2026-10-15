// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from "react";
import { commands, type PlaneId } from "./bindings";
import { listen } from "./here";
import { Notice } from "./Notice";

/** What the core sends as a task's quiet note comes or goes (`silence::TaskQuiet`). */
type TaskQuiet = { plane: PlaneId; session: number; said: string | null };

/**
 * **A task gone quiet, on its tab** (#1981, D-6): purlis watches each task's last activity, and
 * after a quiet stretch says so here and to the chat that asked (`task-quiet`). The note is
 * purlis's own, made from what the task's hooks were heard say, and **it ends nothing**. It comes
 * down by itself when the task is heard from again, and **Dismiss** puts it away.
 */
export function TaskQuietNotice({ plane, session }: { plane: PlaneId; session: number }) {
  const quiet = useTaskQuiet(plane, session);
  if (quiet.said === null) return null;

  const putAway = () => {
    void commands
      .putAwayTaskQuiet(plane, session)
      .catch(() => {})
      .finally(quiet.read);
  };

  return (
    <Notice cause={`task-quiet:${session}`} at="pane" label="Quiet" onDismiss={putAway}>
      purlis notes that {quiet.said}. It is still running: purlis ends nothing for it, and the chat
      that asked was told too.
    </Notice>
  );
}

/**
 * Task `session`'s quiet note, read when the pane mounts and each time the core says it came or
 * went, and `read` to read it again after a press. A note that cannot be read shows nothing: the
 * chat that asked is still told.
 */
export function useTaskQuiet(
  plane: PlaneId,
  session: number,
): { said: string | null; read: () => void } {
  const [said, setSaid] = useState<string | null>(null);
  const read = useCallback(() => {
    void commands
      .taskQuiet(plane, session)
      .then((answer) => {
        if (answer.status === "ok") setSaid(typeof answer.data === "string" ? answer.data : null);
      })
      .catch(() => {});
  }, [plane, session]);
  useEffect(() => {
    read();
    let gone = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await listen<TaskQuiet>("task-quiet", (event) => {
          if (event.payload.plane !== plane || event.payload.session !== session) return;
          read();
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
  }, [plane, session, read]);
  return { said, read };
}
