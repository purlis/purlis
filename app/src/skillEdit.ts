// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A skill's text, edited in place** (#3683; spec #3679, the amendment's S4): what the Skill
 * studio's editor holds, kept by the studio's tab.
 *
 * - **It is the tab's, not its view's.** The tab in front is the only one drawn, so the studio is
 *   drawn afresh each time its tab comes back to the front. What was typed is kept here, by the
 *   tab's history id (`studioHistoryOf`), so it is still in the editor when the person comes
 *   back, and the Text tab's "Unsaved changes" mark still says so. It is kept in memory only, as
 *   the tab's history is (`viewHistory.ts`): a relaunch starts afresh.
 * - **Unsaved, it is registered as the view's unsaved work** (`unsavedWork.ts`, #3929), so every
 *   close that would lose it asks first, the window's one question: a tab's, a pane's, the
 *   project's, the removal of the workspace it is saved into, and a quit.
 * - **A closed tab's edit goes with it** ({@link forgetSkillEdit}), as its history does.
 */
import { useSyncExternalStore } from "react";
import { commands, type SkillLevel, type StudioSkill } from "./bindings";
import type { StudioPlace } from "./skillStudio";
import type { ViewRef } from "./tabs";
import { keepUnsaved } from "./unsavedWork";

/** **The editor in progress**: the skill's page it was opened on, and its text then and now. */
export type SkillEdit = {
  /** The project it is in, and the studio's view that holds it. */
  plane: string;
  view: ViewRef;
  /** The skill's page it was opened at, on its Text tab. */
  at: StudioPlace;
  skill: StudioSkill;
  level: SkillLevel;
  /** The text the editor opened. */
  was: string;
  /** The text as it is typed now. */
  now: string;
};

/** What Save says once the new version is saved (S4). */
export const SAVED = "Saved. Chats started from now get this version.";

/** Whether `edit` holds anything a leave would lose. */
export function unsaved(edit: SkillEdit | undefined): boolean {
  return edit !== undefined && edit.now !== edit.was;
}

const edits = new Map<string, SkillEdit>();
const listeners = new Set<() => void>();

function told(): void {
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** **Says the tab `tab`'s edit to the registry** (#3929): as its unsaved work while it holds
 *  any, else as none. */
function registered(tab: string, edit: SkillEdit | undefined): void {
  if (edit === undefined || !unsaved(edit)) return keepUnsaved(tab, undefined);
  keepUnsaved(tab, {
    key: tab,
    plane: edit.plane,
    view: edit.view,
    workspace: edit.level.at === "workspace" ? edit.level.name : undefined,
    name: edit.skill.name,
    save: async () => {
      // The edit as it is when the person answers, which is the one registered.
      const answer = await commands
        .skillSave(edit.plane, edit.level, edit.skill.name, edit.was, edit.now)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (answer.status === "error") return answer.error;
      if (edits.get(tab) === edit) keepSkillEdit(tab, undefined);
      return undefined;
    },
    discard: () => keepSkillEdit(tab, undefined),
  });
}

/** **Keeps `edit` as the tab `tab`'s editor**, or ends it (`undefined`). */
export function keepSkillEdit(tab: string, edit: SkillEdit | undefined): void {
  if (edit === undefined ? !edits.delete(tab) : edits.get(tab) === edit) return;
  if (edit !== undefined) edits.set(tab, edit);
  registered(tab, edit);
  told();
}

/** The tab `tab`'s editor, as it is this moment. */
export function skillEditIn(tab: string): SkillEdit | undefined {
  return edits.get(tab);
}

/** The tab `tab`'s editor, drawn again as it changes. */
export function useSkillEdit(tab: string): SkillEdit | undefined {
  return useSyncExternalStore(subscribe, () => edits.get(tab));
}

/** **The tab `tab` has closed**: its editor goes with it. */
export function forgetSkillEdit(tab: string): void {
  keepSkillEdit(tab, undefined);
}
