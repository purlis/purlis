// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type PlaneId } from "../bindings";
import { Field } from "./components";
import type { LiveSetting } from "./groups";

/** The row's address. */
export const HIBERNATE_AFTER = "project.chats.hibernate-after";

/**
 * **Settings › Project › Chats › Hibernate after** (SC-4h, #7637): `[chats] hibernate_after`,
 * how long a chat of this project nobody looks at rests before purlis may hibernate it, while
 * this device has more chats running than its target. The person ruled on 2026-11-25: 30 minutes
 * by default, `off` allowed, nothing under 10 minutes.
 *
 * - **This machine's alone**: the core writes the project's local settings file, never the
 *   committed one (`purlis_core::hibernateafter::set`). The help never names that file: only the
 *   core knows which it is.
 * - **What is drawn is what the core read** (`hibernate_after`). A value is sent once it is left
 *   or Enter is pressed, and the core checks it: one it refuses is said under the box, which keeps
 *   the words, and nothing is written. The window checks nothing itself.
 */
export function hibernateAfterSetting(plane: PlaneId): LiveSetting {
  return {
    id: HIBERNATE_AFTER,
    label: "Hibernate after",
    help: "When this device has more chats running than it is meant to hold, purlis hibernates a chat of this project once nobody has looked at it or typed in it for this long, the longest-resting first. A hibernated chat resumes where it was when you open it. Write a time such as 30m or 2h, at least 10 minutes, or off to hibernate none of them by themselves. Hibernate on a chat's row still works.",
    writes: "local",
    useControl: function useHibernateAfter() {
      const [read, setRead] = useState<string>();
      const [typed, setTyped] = useState("");
      const [refused, setRefused] = useState<string>();
      useEffect(() => {
        let live = true;
        void commands
          .hibernateAfter(plane)
          .then((said) => {
            if (!live) return;
            if (said.status === "ok") {
              setRead(said.data);
              setTyped(said.data);
            } else setRefused(said.error);
          })
          .catch((err: unknown) => {
            if (live) setRefused(String(err));
          });
        return () => {
          live = false;
        };
      }, []);
      const commit = () => {
        if (read === undefined || typed.trim() === read) return;
        void commands
          .setHibernateAfter(plane, typed)
          .then((said) => {
            if (said.status === "ok") {
              setRead(said.data);
              setTyped(said.data);
              setRefused(undefined);
            } else setRefused(said.error);
          })
          .catch((err: unknown) => setRefused(String(err)));
      };
      return {
        error: refused === undefined ? undefined : [refused],
        control: (ids) => (
          <Field
            kind="text"
            ids={ids}
            value={typed}
            placeholder="30m"
            size={8}
            disabled={read === undefined}
            onChange={setTyped}
            onCommit={commit}
          />
        ),
      };
    },
  };
}
