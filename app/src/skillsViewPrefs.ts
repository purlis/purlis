// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { layoutPref } from "./layoutPref";

/**
 * **What the Skills view keeps for the window** (SK-55, #3266; #3308): whether its list is
 * narrowed to the chat in front, Only this chat's (the decisions amendment's (c)). One switch for
 * the window, whichever project is in front.
 *
 * **Kept in the layout file, under `skills`** (`layout.json`, `regions.ts`), beside the window's
 * other choices on this machine, so it survives a restart as the Chats list's drawing does. Only
 * a narrowed list is written, as `{ "onlyThisChat": true }`: the whole list is the default, which
 * the file leaves out.
 */
export type SkillsViewPrefs = { onlyThisChat: boolean };

export const DEFAULT_SKILLS_VIEW: SkillsViewPrefs = { onlyThisChat: false };

/** The preferences as a layout document holds them, and what had to be put right. */
export function loadSkillsView(raw: unknown): { value: SkillsViewPrefs; said: string[] } {
  const held =
    raw !== null && typeof raw === "object" && !Array.isArray(raw)
      ? (raw as { skills?: unknown }).skills
      : undefined;
  if (held === undefined) return { value: DEFAULT_SKILLS_VIEW, said: [] };
  const only =
    held !== null && typeof held === "object" && !Array.isArray(held)
      ? (held as { onlyThisChat?: unknown }).onlyThisChat
      : null;
  if (only === undefined || only === false) return { value: DEFAULT_SKILLS_VIEW, said: [] };
  if (only === true) return { value: { onlyThisChat: true }, said: [] };
  return {
    value: DEFAULT_SKILLS_VIEW,
    said: [
      `"skills" ${JSON.stringify(held)} is not the Skills view's choices, so it lists every skill`,
    ],
  };
}

/** The store (`layoutPref.ts`), under `skills`. */
export const SKILLS_VIEW = layoutPref<SkillsViewPrefs, "skills">({
  key: "skills",
  fallback: DEFAULT_SKILLS_VIEW,
  load: loadSkillsView,
  same: (one, other) => one.onlyThisChat === other.onlyThisChat,
  written: (prefs) => (prefs.onlyThisChat ? { onlyThisChat: true } : undefined),
  remedy: (where) =>
    `fix ${where}, or press Only this chat's in the Skills view, which rewrites it`,
});

/** Whether the list is narrowed to the chat in front, and the way to change it. */
export function useOnlyThisChat(): [boolean, (only: boolean) => void] {
  return [SKILLS_VIEW.use().onlyThisChat, setOnlyThisChat];
}

function setOnlyThisChat(only: boolean): void {
  SKILLS_VIEW.set({ onlyThisChat: only });
}
