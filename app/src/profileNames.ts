// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useState } from "react";
import { commands, type PlaneId, type SettingsFile } from "./bindings";

/** Every project's harness profiles, by name: a list per project that has any. */
export type ProfileNames = Readonly<Record<PlaneId, readonly string[]>>;

const NONE: ProfileNames = {};

/**
 * **The projects' harness profiles, by name, for the palette's rows to their pages** (#1201,
 * D-1201-4): one Settings row each, "Project settings: Profile claude".
 *
 * **Read when the palette opens over a project (`read`), never on a render of its own**: a
 * profile is a table in the Local file, and reading the files on every draw of the window would
 * be a command per draw for rows nobody is looking at. Each opening reads them again, so a
 * profile added since is there. Named as Settings names its pages (`settings/project.ts`,
 * `profilePages`): an entry's `name` value, else its label. A file the core could not parse
 * lists none.
 *
 * **Held by the window, and the same lists while the names are** (#4351): a project's view is
 * given only its own list, so the palette opening or closing changes nothing it is drawn from,
 * and a read that finds the same names keeps the list it had. A project with no profiles is
 * given no list at all, so reading one, the usual project, draws nothing again.
 */
export function useProfileNames(): { of: ProfileNames; read: (plane: PlaneId) => void } {
  const [of, setOf] = useState<ProfileNames>(NONE);
  const read = useCallback((plane: PlaneId) => {
    void commands
      .projectSettings(plane)
      .then((answer) => {
        if (answer.status !== "ok") return;
        const next = profileNamesOf(answer.data?.local);
        setOf((was) => {
          const held = was[plane] ?? [];
          if (held.length === next.length && held.every((name, at) => name === next[at]))
            return was;
          if (next.length > 0) return { ...was, [plane]: next };
          return Object.fromEntries(Object.entries(was).filter(([at]) => at !== plane));
        });
      })
      .catch(() => {
        // No core to ask (a test, a window going away): the rows wait for the next opening.
      });
  }, []);
  return { of, read };
}

/** The profiles a Local file lists, by the names their pages have. */
export function profileNamesOf(local: SettingsFile | undefined): readonly string[] {
  if (local === undefined || !local.parsed) return [];
  return (local.entries ?? [])
    .filter((one) => one.collection === "profiles")
    .map((one) => one.values.find((value) => value.field === "name")?.value ?? one.label)
    .filter((name): name is string => typeof name === "string" && name !== "");
}
