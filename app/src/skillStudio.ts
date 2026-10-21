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

/**
 * **Whose chats the studio shows** (#2767): a chat at the project's root, a chat in one
 * workspace, or a chat as one persona (#2768), which brings its skills into every workspace.
 */
export type StudioScope =
  { at: "root" } | { at: "workspace"; workspace: string } | { at: "persona"; persona: string };

/** A chat at the project's root. */
export const AT_THE_ROOT: StudioScope = { at: "root" };

/** A chat in `workspace`. */
export function inWorkspace(workspace: string): StudioScope {
  return { at: "workspace", workspace };
}

/** A chat as `persona`, at the project's root (#2768). */
export function asPersona(persona: string): StudioScope {
  return { at: "persona", persona };
}

/** The workspace a scope names, for the core, which takes none for the project's root. */
export function workspaceOf(scope: StudioScope): string | null {
  return scope.at === "workspace" ? scope.workspace : null;
}

/** The persona a scope names, for the core, which takes none for a chat on no persona. */
export function personaOf(scope: StudioScope): string | null {
  return scope.at === "persona" ? scope.persona : null;
}

/**
 * What a persona's studio view is keyed by: `persona/<name>`. A workspace's name is one plain
 * name, so it never holds the `/`.
 */
const PERSONA_KEY = "persona/";

/**
 * The scope a studio view shows: its key is the workspace, or `persona/<name>`, and the view's
 * own key the root.
 */
export function scopeOf(view: ViewRef): StudioScope {
  if (view.key === SKILL_STUDIO_VIEW.key) return AT_THE_ROOT;
  if (view.key.startsWith(PERSONA_KEY)) return asPersona(view.key.slice(PERSONA_KEY.length));
  return inWorkspace(view.key);
}

/**
 * **The Skill studio on one scope's chats** (#2767): the same view, keyed by the workspace, so
 * a Skills summary row opens it on that workspace and the studio's own picker can show another.
 */
export function skillStudioView(scope: StudioScope): ViewRef {
  if (scope.at === "root") return SKILL_STUDIO_VIEW;
  if (scope.at === "persona")
    return { ...SKILL_STUDIO_VIEW, key: `${PERSONA_KEY}${scope.persona}` };
  return { ...SKILL_STUDIO_VIEW, key: scope.workspace };
}

/** What the studio's tab is called while it shows `scope`'s chats. */
export function skillStudioTitle(scope: StudioScope): string {
  if (scope.at === "root") return SKILL_STUDIO_TITLE;
  if (scope.at === "persona") return `${SKILL_STUDIO_TITLE} · ${scope.persona}`;
  return `${SKILL_STUDIO_TITLE} · ${scope.workspace}`;
}

/** The catalogue row that opens the studio on `workspace`'s chats (#2767). */
export const SKILL_STUDIO_IN = "skills.studio:";

/** The catalogue row that opens the studio on `persona`'s chats (#2768): what its page's rows run. */
export function skillStudioAs(persona: string): string {
  return `${SKILL_STUDIO_IN}${PERSONA_KEY}${persona}`;
}

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
