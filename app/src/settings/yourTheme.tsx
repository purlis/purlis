// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { commands, type PlaneId, type ProjectTheme } from "../bindings";
import { projectThemeChanged } from "../projectTheme";
import { Choice } from "./components";
import type { LiveSetting } from "./groups";

/**
 * **Your theme here** (#2323): the person's own choice of theme and appearance for this project,
 * or for this workspace in it, kept on this machine by the project's id and never in a project
 * file (`purlis_core::theme::choices`, written by `theme_choose`). It is the nearest level the
 * theme engine reads, so it beats the project's and a LIVE workspace's committed default; the
 * row says which level the theme drawn here came from, as the engine resolved it
 * (`ProjectTheme.level`, `from`).
 */
export function yourTheme(
  plane: PlaneId,
  workspace: string | undefined,
  theme: ProjectTheme | undefined,
  reread: () => void,
): LiveSetting[] {
  const where = workspace === undefined ? "this project" : "this workspace";
  const scope = workspace === undefined ? "project" : "workspace";
  const yours = theme?.yours ?? null;
  const options = [
    ...(theme?.folders ?? []).map((value) => ({ value, label: `${value} (theme folder)` })),
    ...(theme?.options ?? []),
  ];
  const drawn =
    theme?.drawn == null ? "" : ` Drawn here: ${theme.drawn}, from ${theme.from ?? "nothing"}.`;
  const choose = (to: string | null, appearance: string | null) =>
    commands.themeChoose(plane, workspace ?? null, to, appearance).then((done) => {
      projectThemeChanged(plane);
      reread();
      return done;
    });
  return [
    {
      id: `${scope}.appearance.yours`,
      label: "Your theme",
      help: `Your own theme for ${where}, kept on this machine: it comes before what the project or a LIVE workspace commits.${drawn}`,
      writes: "this machine",
      useControl: function useYourTheme() {
        const [error, setError] = useState<readonly string[]>();
        return {
          error,
          control: (ids) => (
            <Choice
              kind="select"
              ids={ids}
              options={options}
              value={yours?.theme ?? undefined}
              unset="not chosen — the committed default"
              onValueChange={(to) =>
                void choose(to === "" ? null : to, yours?.appearance ?? null).then((done) =>
                  setError(done.status === "error" ? [done.error] : undefined),
                )
              }
            />
          ),
        };
      },
    },
    {
      id: `${scope}.appearance.yours-appearance`,
      label: "Your appearance",
      help: `Which of your theme's appearances ${where} draws: follow the system, or one by its id.`,
      writes: "this machine",
      useControl: function useYourAppearance() {
        const [error, setError] = useState<readonly string[]>();
        return {
          error,
          control: (ids) => (
            <Choice
              kind="select"
              ids={ids}
              disabled={yours === null}
              options={[
                { value: "system", label: "Follow the system" },
                { value: "dark", label: "dark" },
                { value: "light", label: "light" },
              ]}
              value={yours?.appearance ?? undefined}
              unset="the theme's first"
              onValueChange={(to) =>
                yours !== null &&
                void choose(yours.theme, to === "" ? null : to).then((done) =>
                  setError(done.status === "error" ? [done.error] : undefined),
                )
              }
            />
          ),
        };
      },
    },
  ];
}
