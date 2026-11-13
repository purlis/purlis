// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import {
  commands,
  TASK_SCREENS_KEPT_FOR_DAYS,
  TASK_SCREENS_MOST_KEPT,
  TASK_SCREENS_MOST_KEPT_MIB,
  type PlaneId,
  type TaskScreens,
} from "../bindings";
import { AlertDialog } from "../ui/dialog/Dialog";
import { Choice } from "./components";
import type { LiveSetting } from "./groups";

/** The row's address, on the project's Dispatch page. */
export const TASK_SCREENS = "project.dispatch.task-screens";

/**
 * **Settings › Project › Dispatch › Keep task screens** (#6306): `[task_screens] keep`, which
 * the person ruled on 2026-11-11 (#6214). A dispatched task's last screen is kept as its chat
 * closes, for *Read it* on its card. The row says what is kept, where, and for how long.
 *
 * - **This machine's alone**: the core writes `purlis.local.toml`, never the committed file
 *   (`purlis_core::chatscreens::set_keep`), as `[skill_grades]` is.
 * - **On writes at once; off asks first**, since it deletes every kept screen, and nothing puts
 *   them back. The ask says how many go.
 * - **What is drawn is what the core read** (`task_screens`), and a hand edit of the file is caught
 *   there: off by hand, the core deletes the screens as it reads it.
 */
export function taskScreensSetting(plane: PlaneId): LiveSetting {
  return {
    id: TASK_SCREENS,
    label: "Keep task screens",
    section: {
      label: "On this machine",
      help: "Kept in purlis.local.toml, which only this machine reads.",
    },
    help: `When a dispatched task's chat closes, purlis keeps its last screen and scrollback, so Read it on the chat's card can show it. Never your own chats. Kept on this machine only: set in purlis.local.toml, kept in the project's state folder, out of every chat's reach. It keeps at most ${TASK_SCREENS_MOST_KEPT} screens and ${TASK_SCREENS_MOST_KEPT_MIB} MiB, deleting the oldest first. When the project opens, a screen is deleted ${TASK_SCREENS_KEPT_FOR_DAYS} days after it was kept, or once its task's record is gone. Turning it off deletes every kept screen.`,
    writes: "local",
    useControl: function useTaskScreens() {
      const [read, setRead] = useState<TaskScreens>();
      const [refused, setRefused] = useState<string>();
      const [asking, setAsking] = useState(false);
      useEffect(() => {
        let live = true;
        void commands.taskScreens(plane).then((said) => {
          if (!live) return;
          if (said.status === "ok") setRead(said.data);
          else setRefused(said.error);
        });
        return () => {
          live = false;
        };
      }, []);
      const set = (keep: boolean) => {
        setAsking(false);
        void commands
          .setTaskScreens(plane, keep)
          .then((said) => {
            if (said.status === "ok") {
              setRead(said.data);
              setRefused(undefined);
              return;
            }
            setRefused(said.error);
            // Off can be written and still leave screens it could not delete: the row then
            // shows the setting and the count as they are, not as they were.
            return commands.taskScreens(plane).then((now) => {
              if (now.status === "ok") setRead(now.data);
            });
          })
          .catch((err: unknown) => setRefused(String(err)));
      };
      const kept = read?.kept ?? 0;
      return {
        error: refused === undefined ? undefined : [refused],
        control: (ids) => (
          <>
            <Choice
              kind="switch"
              ids={ids}
              checked={read?.keep ?? true}
              disabled={read === undefined}
              onCheckedChange={(to) => (to ? set(true) : setAsking(true))}
            />
            {asking && (
              <StopKeepingTaskScreens
                kept={kept}
                onCancel={() => setAsking(false)}
                onDelete={() => set(false)}
              />
            )}
          </>
        ),
      };
    },
  };
}

/**
 * **The ask before keeping is turned off** (#6306): how many kept screens go, that nothing is
 * kept from then on, and that each task's report stays. *Delete kept screens* is the danger
 * answer, since nothing puts them back.
 */
export function StopKeepingTaskScreens({
  kept,
  onDelete,
  onCancel,
}: {
  kept: number;
  onDelete: () => void;
  onCancel: () => void;
}) {
  return (
    <AlertDialog
      onClose={onCancel}
      title="Stop keeping task screens?"
      description={
        <p>
          {kept === 0
            ? "No screen is kept now. "
            : `purlis deletes the ${kept} kept ${kept === 1 ? "screen" : "screens"} now. `}
          From then on no task's screen is kept, so Read it on a closed task's card has nothing to
          show. Its report stays.
        </p>
      }
      main={{ children: "Delete kept screens", variant: "danger", onPress: onDelete }}
    />
  );
}
