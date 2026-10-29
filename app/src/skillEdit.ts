// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A skill's text, edited in place** (#3683; spec #3679, the amendment's S4): what the Skill
 * studio's editor holds, kept by the studio's tab, and a close of that tab waiting on the
 * person's answer.
 *
 * - **It is the tab's, not its view's.** The tab in front is the only one drawn, so the studio is
 *   drawn afresh each time its tab comes back to the front. What was typed is kept here, by the
 *   tab's history id (`studioHistoryOf`), so it is still in the editor when the person comes
 *   back, and the Text tab's "Unsaved changes" mark still says so. It is kept in memory only, as
 *   the tab's history is (`viewHistory.ts`): a relaunch starts afresh.
 * - **A close of the tab asks first** where the edit is unsaved ({@link askBeforeClosing}): the
 *   window brings the tab to the front, and the studio asks Keep editing, Discard and leave, or
 *   Save and leave, then carries the close out or not.
 * - **A closed tab's edit goes with it** ({@link forgetSkillEdit}), as its history does.
 */
import { useSyncExternalStore } from "react";
import type { SkillLevel, StudioSkill } from "./bindings";
import { isSkillStudio, studioHistoryOf, type StudioPlace } from "./skillStudio";
import { contentsOf, type Tabs } from "./tabs";
import type { Offer } from "./actions";

/** **The editor in progress**: the skill's page it was opened on, and its text then and now. */
export type SkillEdit = {
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
const closes = new Map<string, () => void>();
const listeners = new Set<() => void>();

function told(): void {
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** **Keeps `edit` as the tab `tab`'s editor**, or ends it (`undefined`). */
export function keepSkillEdit(tab: string, edit: SkillEdit | undefined): void {
  if (edit === undefined ? !edits.delete(tab) : edits.get(tab) === edit) return;
  if (edit !== undefined) edits.set(tab, edit);
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

/**
 * **A close of the tab `tab`, asked about first** where its editor holds unsaved text: `close` is
 * kept until the person answers, and `true` says so. `false` where nothing would be lost, and the
 * close goes on at once.
 */
export function askBeforeClosing(tab: string, close: () => void): boolean {
  if (!unsaved(edits.get(tab))) return false;
  closes.set(tab, close);
  told();
  return true;
}

/** The close of the tab `tab` that waits on the person's answer, while one does. */
export function useCloseAsked(tab: string): (() => void) | undefined {
  return useSyncExternalStore(subscribe, () => closes.get(tab));
}

/** The close of the tab `tab` is answered: it is no longer waiting. */
export function closeAnswered(tab: string): void {
  if (closes.delete(tab)) told();
}

/** **The tab `tab` has closed**: its editor goes with it. */
export function forgetSkillEdit(tab: string): void {
  const had = edits.delete(tab);
  if (closes.delete(tab) || had) told();
}

/**
 * **The studio tab a close would lose unsaved text in**, by its history id and the window tab
 * that shows it: a tab's close (`closeTab`), or a pane's in the tab in front (`closePane`).
 */
export function unsavedClosedBy(
  plane: string,
  tabs: Tabs,
  does: Offer["does"],
): { history: string; tab: number } | undefined {
  const tab =
    does.verb === "closeTab"
      ? does.tab
      : does.verb === "closePane"
        ? tabs.inFront
        : undefined;
  if (tab === undefined) return undefined;
  const focused = tabs.byId[tab]?.focused;
  for (const { pane, content } of contentsOf(tabs, tab)) {
    if (does.verb === "closePane" && pane !== focused) continue;
    if (content.kind !== "view" || !isSkillStudio(content.view)) continue;
    const history = studioHistoryOf(plane, content.view);
    if (unsaved(edits.get(history))) return { history, tab };
  }
  return undefined;
}
