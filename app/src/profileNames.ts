import { useEffect, useState } from "react";
import { commands, type PlaneId, type SettingsFile } from "./bindings";

/**
 * **The project's harness profiles, by name, for the palette's rows to their pages** (#1201,
 * D-1201-4): one Settings row each, "Project settings: Profile claude".
 *
 * **Read when the palette opens, never on a render of its own**: a profile is a table in the
 * Local file, and reading the files on every draw of the window would be a command per draw for
 * rows nobody is looking at. Each opening reads them again, so a profile added since is there.
 * Named as Settings names its pages (`settings/project.ts`, `profilePages`): an entry's `name`
 * value, else its label. A file the core could not parse lists none.
 */
export function useProfileNames(
  plane: PlaneId,
  paletteOpen: boolean,
): readonly string[] | undefined {
  const [names, setNames] = useState<readonly string[]>();
  useEffect(() => {
    if (!paletteOpen) return;
    let gone = false;
    void commands
      .projectSettings(plane)
      .then((read) => {
        if (gone || read.status !== "ok") return;
        setNames(profileNamesOf(read.data?.local));
      })
      .catch(() => {
        // No core to ask (a test, a window going away): the rows wait for the next opening.
      });
    return () => {
      gone = true;
    };
  }, [plane, paletteOpen]);
  return names;
}

/** The profiles a Local file lists, by the names their pages have. */
export function profileNamesOf(local: SettingsFile | undefined): readonly string[] {
  if (local === undefined || !local.parsed) return [];
  return (local.entries ?? [])
    .filter((one) => one.collection === "profiles")
    .map((one) => one.values.find((value) => value.field === "name")?.value ?? one.label)
    .filter((name): name is string => typeof name === "string" && name !== "");
}
