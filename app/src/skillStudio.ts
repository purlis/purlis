// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useSyncExternalStore } from "react";
import { commands, PERSONA_SCOPE_KEY, SKILL_STUDIO_IN, type SkillPinner } from "./bindings";
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

/** Whose chats the studio shows (#2767, #2768): at the project's root, in a workspace, or as a
 *  persona, which brings its skills into every workspace. */
export type StudioAt = "root" | "workspace" | "persona";

/** **Whose chats the studio shows**: `name` is the workspace's or the persona's, empty at the root. */
export type StudioScope = { at: StudioAt; name: string };

/**
 * **Each kind of scope, in one place**: the prefix of its view key (and so of the studio's
 * picker's value, which is the same key), what its tab is called, and what the studio says of
 * whose chats it shows. A workspace's name is one plain name and never holds the `/` a persona's
 * key starts with.
 */
const SCOPES: Record<
  StudioAt,
  { prefix: string; title: (name: string) => string; chat: (name: string) => string }
> = {
  root: { prefix: "", title: () => SKILL_STUDIO_TITLE, chat: () => "A chat at the project's root" },
  workspace: {
    prefix: "",
    title: (name) => `${SKILL_STUDIO_TITLE} · ${name}`,
    chat: (name) => `A chat in ${name}`,
  },
  persona: {
    prefix: PERSONA_SCOPE_KEY,
    title: (name) => `${SKILL_STUDIO_TITLE} · ${name}`,
    chat: (name) => `A chat as ${name}`,
  },
};

/** A chat at the project's root. */
export const AT_THE_ROOT: StudioScope = { at: "root", name: "" };

/** A chat in `workspace`. */
export function inWorkspace(workspace: string): StudioScope {
  return { at: "workspace", name: workspace };
}

/** A chat as `persona`, at the project's root (#2768). */
export function asPersona(persona: string): StudioScope {
  return { at: "persona", name: persona };
}

/** The workspace and the persona a scope names, as the core takes them: `null` for none. */
export function whoseChats(scope: StudioScope): {
  workspace: string | null;
  persona: string | null;
} {
  return {
    workspace: scope.at === "workspace" ? scope.name : null,
    persona: scope.at === "persona" ? scope.name : null,
  };
}

/** What the studio's view is keyed by for `scope`: the one encoding of a scope. */
export function scopeKey(scope: StudioScope): string {
  return `${SCOPES[scope.at].prefix}${scope.name}`;
}

/** The scope a studio view's key names. */
export function scopeOfKey(key: string): StudioScope {
  if (key === "") return AT_THE_ROOT;
  const persona = SCOPES.persona.prefix;
  return key.startsWith(persona) ? asPersona(key.slice(persona.length)) : inWorkspace(key);
}

/** The scope a studio view shows. */
export function scopeOf(view: ViewRef): StudioScope {
  return scopeOfKey(view.key);
}

/**
 * **The Skill studio on one scope's chats** (#2767): the same view, keyed by its scope, so a
 * Skills summary row opens it on that workspace or persona and the studio's own picker can show
 * another.
 */
export function skillStudioView(scope: StudioScope): ViewRef {
  return { ...SKILL_STUDIO_VIEW, key: scopeKey(scope) };
}

/** What the studio's tab is called while it shows `scope`'s chats. */
export function skillStudioTitle(scope: StudioScope): string {
  return SCOPES[scope.at].title(scope.name);
}

/** Whose chat `scope` is, as the studio's picker names it. */
export function scopeChat(scope: StudioScope): string {
  return SCOPES[scope.at].chat(scope.name);
}

/** The catalogue row that opens the studio on a scope's chats, before the scope's key (#2767): the
 *  core names it once (`skillstudio::STUDIO_IN`). */
export { SKILL_STUDIO_IN };

/** The catalogue row that opens the studio on `persona`'s chats (#2768): what its page's summary
 *  row and a chat's missing-skills Notice run. */
export function skillStudioAs(persona: string): string {
  return `${SKILL_STUDIO_IN}${scopeKey(asPersona(persona))}`;
}

/** **What opens the studio, in words**: the palette's row and the Skills view's header (#3266). */
export const OPEN_SKILL_STUDIO = "Open Skill studio";

/** The catalogue row that opens the studio on a chat at the project's root (#2765). */
export const SKILL_STUDIO_SHOW = "skills.studio";

/** Whether `view` is the Skill studio. */
export function isSkillStudio(view: ViewRef): boolean {
  return view.from === null && view.view === SKILL_STUDIO_VIEW.view;
}

/** What this machine said of the flag, once asked, and who is drawn from it. */
let said: boolean | undefined;
/** Whether the last ask was refused or failed: the preview counts as off until an ask is
 *  answered, so nothing waits on it (#3308). */
let unanswered = false;
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
      unanswered = answer.status !== "ok";
    })
    .catch(() => {
      unanswered = true;
    })
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

/** What the core said of the Skill studio's preview: `undefined` until it has, which a view
 *  that would otherwise draw something else for a moment holds on (#3308); `false` where it
 *  could not say, as the preview is then drawn off. */
export function useSkillStudioSaid(): boolean | undefined {
  return useSyncExternalStore(subscribe, () => said ?? (unanswered ? false : undefined));
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

/**
 * **A start the studio is asked to open on** (#3266): the Skills view's New skill and Browse
 * sources open the studio's own, so each start is the studio's one copy. `new` is the New skill
 * form, `sources` its skill sources.
 */
export type StudioStart = "new" | "sources";

/** The start asked for, by the studio's view key, until a studio of that key takes it. */
let startAsked: { key: string; start: StudioStart } | undefined;
const startListeners = new Set<() => void>();

/** Asks the studio on `scope`'s chats to open on `start`: the one drawn now, or the next. */
export function askStudioStart(scope: StudioScope, start: StudioStart): void {
  startAsked = { key: scopeKey(scope), start };
  for (const one of startListeners) one();
}

/**
 * **Hands the studio on `scope`'s chats each start asked of it** (#3266), once it is `ready`
 * (its skills read): one asked before it was drawn, then each asked while it is. A start is taken
 * once, so drawing the studio again opens nothing.
 */
export function useStudioStart(
  scope: StudioScope,
  ready: boolean,
  onStart: (start: StudioStart) => void,
): void {
  const key = scopeKey(scope);
  const held = useRef(onStart);
  useEffect(() => {
    held.current = onStart;
  });
  useEffect(() => {
    if (!ready) return;
    const take = () => {
      if (startAsked?.key !== key) return;
      const { start } = startAsked;
      startAsked = undefined;
      held.current(start);
    };
    startListeners.add(take);
    // One asked before this studio was drawn, taken after this render.
    void Promise.resolve().then(take);
    return () => {
      startListeners.delete(take);
    };
  }, [key, ready]);
}

/** **The studio a pin is changed in** (SK-15d): its workspace's chats, or its persona's. */
export function scopeOfPinner(by: SkillPinner): StudioScope {
  return by.at === "workspace" ? inWorkspace(by.name) : asPersona(by.name);
}

/** A pinner as a sentence names it: `workspace ops`, `persona reviewer`. */
export function pinnerSaid(by: SkillPinner): string {
  return `${by.at} ${by.name}`;
}

/** **The pinner the studio of `scope` pins for**, or `null` at the project's root (SK-15d). */
export function pinnerOf(scope: StudioScope): SkillPinner | null {
  if (scope.at === "root") return null;
  return { at: scope.at, name: scope.name };
}
