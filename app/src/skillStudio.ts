// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";
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

/** What this machine said of the flag, once asked, and who is drawn from it. */
let said: boolean | undefined;
let asking = false;
const listeners = new Set<() => void>();

function told() {
  for (const one of listeners) one();
}

function ask() {
  if (asking) return;
  asking = true;
  void commands
    .skillStudioOn()
    .then((answer) => {
      if (answer.status === "ok") said = answer.data;
    })
    .catch(() => undefined)
    .finally(() => {
      asking = false;
      told();
    });
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  if (said === undefined) ask();
  return () => {
    listeners.delete(listener);
  };
}

/** Whether the Skill studio is on in this window: `false` until the core has said. */
export function useSkillStudioOn(): boolean {
  return useSyncExternalStore(subscribe, () => said ?? false);
}

/** **Turns the Skill studio's preview on or off on this machine** (Settings › You ›
 *  Previews), then draws what the core says is now in force, or the core's refusal. */
export async function setSkillStudioOn(on: boolean): Promise<string | undefined> {
  const answer = await commands
    .setSkillStudioOn(on)
    .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
  if (answer.status === "ok") said = answer.data;
  told();
  return answer.status === "error" ? answer.error : undefined;
}
