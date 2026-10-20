// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands } from "./bindings";
import type { ViewRef } from "./tabs";

/**
 * **The Skill studio** (SK-2, #2765; spec #2762): every skill a chat in the project gets, with
 * its level, and every one left out with why, read only for now.
 *
 * It is a view tab, `{ from: null, view: "skill-studio", key: "" }`: one per project, opened
 * from the palette's `skills.studio` row. It is behind the `skill-studio` flag until its finish
 * line (#2786): while the flag is off the palette offers no row for it.
 */
export const SKILL_STUDIO_VIEW: ViewRef = { from: null, view: "skill-studio", key: "" };

/** What the Skill studio's tab is called. */
export const SKILL_STUDIO_TITLE = "Skill studio";

/** Whether `view` is the Skill studio. */
export function isSkillStudio(view: ViewRef): boolean {
  return view.from === null && view.view === SKILL_STUDIO_VIEW.view;
}

/** What this machine said of the flag, once asked: it changes only with a restart. */
let said: boolean | undefined;

/** Whether the Skill studio is on in this window: `false` until the core has said. */
export function useSkillStudioOn(): boolean {
  const [on, setOn] = useState(said ?? false);
  useEffect(() => {
    if (said !== undefined) return;
    let live = true;
    void commands
      .skillStudioOn()
      .then((answer) => {
        if (answer.status !== "ok") return;
        said = answer.data;
        if (live) setOn(answer.data);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, []);
  return on;
}
