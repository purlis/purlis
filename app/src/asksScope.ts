// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";
import type { AskedBy } from "./tabs";

/**
 * **Which asks Asks lists** (ADR 0121, amended 2026-11-12, decisions 2 to 5): the focused chat's
 * and those of every chat it dispatched (Chat), the workspace in front's (Workspace), or every
 * workspace's of the project in front (Project). Another project's asks are answered in its own
 * Asks, never here. The shared lists that keep an All of their own (the Browsers grid, Updates'
 * switch) read `inboxScope.ts`.
 */
export type AsksScope = "chat" | "workspace" | "project";

/** The chips, as they say themselves, narrowest first. */
export const ASKS_SCOPES: readonly { scope: AsksScope; says: string; of: string }[] = [
  { scope: "chat", says: "Chat", of: "this chat" },
  { scope: "workspace", says: "Workspace", of: "this workspace" },
  { scope: "project", says: "Project", of: "this project" },
];

/** What a scope is called in a sentence: "this workspace". */
export const asksScopeOf = (scope: AsksScope): string =>
  ASKS_SCOPES.find((one) => one.scope === scope)?.of ?? "";

/**
 * **How many asks each chip lists.** Workspace is left out where the project has one workspace:
 * it would list what Project lists.
 */
export type AsksCounts = { chat: number; workspace?: number; project: number };

/**
 * **Chat's set** (decision 2): whether chat `session` is the focused chat or one it dispatched,
 * however deep, by the task links the window holds (`askedBy`), never by anything a chat says of
 * itself. Nothing is in it with no chat focused. A lineage that loops is cut.
 */
export function inChatOf(
  focused: number | undefined,
  askedBy: AskedBy,
): (session: number) => boolean {
  return (session) => {
    if (focused === undefined) return false;
    const seen = new Set<number>();
    for (let at: number | undefined = session; at !== undefined; at = askedBy(at)) {
      if (at === focused) return true;
      if (seen.has(at)) return false;
      seen.add(at);
    }
    return false;
  };
}

/** What tells which scopes hold an ask: Chat's set, and the workspace in front. */
export type AsksSets<A> = {
  inChat: (ask: A) => boolean;
  /** Whether an ask's chat is filed in the workspace in front; left out with one workspace. */
  inHere?: (ask: A) => boolean;
};

/**
 * **Whether `scope` lists an ask**: Chat those in Chat's set; Workspace those filed in the
 * workspace in front and those in Chat's set, wherever a task of the focused chat was filed, so
 * the focused chat's line of work is never listed under Chat and missing from Workspace;
 * Project every one.
 */
export function scopeHolds<A>(scope: AsksScope, { inChat, inHere }: AsksSets<A>) {
  return (ask: A): boolean =>
    scope === "project" || inChat(ask) || (scope === "workspace" && inHere?.(ask) !== false);
}

/**
 * **Whether `scope` lists a Notice or an update under the chips** (decision 9): one about a chat
 * wherever {@link scopeHolds} would list that chat's ask; one about no chat of this project (a
 * persona's tools, the doctor, a stale clone, another project's chat) only under Project.
 */
export function scopeHoldsAbout<A>(scope: AsksScope, sets: AsksSets<A>) {
  const holds = scopeHolds(scope, sets);
  return (about: A | undefined): boolean =>
    about === undefined ? scope === "project" : holds(about);
}

/**
 * **The chips' counts** (decision 5), from the project's asks, each what {@link scopeHolds}
 * lists, Workspace left out without `inHere`. So the counts nest: Chat ≤ Workspace ≤ Project.
 */
export function scopeCounts<A>(asks: readonly A[], sets: AsksSets<A>): AsksCounts {
  const count = (scope: AsksScope) => asks.filter(scopeHolds(scope, sets)).length;
  return {
    chat: count("chat"),
    ...(sets.inHere === undefined ? {} : { workspace: count("workspace") }),
    project: asks.length,
  };
}

/** Whether `scope` can be listed: Chat needs a chat focused, Workspace to be offered. */
const listable = (scope: AsksScope, counts: AsksCounts, focused: boolean): boolean =>
  scope === "chat" ? focused : counts[scope] !== undefined;

/**
 * **The scope Asks is drawn in**: the one picked where it can be listed, else the next wider one
 * that can (Chat with no chat focused lists Workspace, or Project with one workspace).
 */
export function asksScopeDrawn(picked: AsksScope, counts: AsksCounts, focused: boolean): AsksScope {
  const at = ASKS_SCOPES.findIndex((one) => one.scope === picked);
  return (
    ASKS_SCOPES.slice(at).find((one) => listable(one.scope, counts, focused))?.scope ?? "project"
  );
}

/**
 * **The nearest wider scope with something waiting**, for an empty scope's way out: Chat, then
 * Workspace, then Project. Nothing where no wider scope lists anything. It is offered, never
 * taken by itself (decision 4).
 */
export function asksWiderWaiting(
  scope: AsksScope,
  counts: AsksCounts,
  focused: boolean,
): { scope: AsksScope; count: number } | undefined {
  const at = ASKS_SCOPES.findIndex((one) => one.scope === scope);
  for (const one of ASKS_SCOPES.slice(at + 1)) {
    const count = counts[one.scope];
    if (listable(one.scope, counts, focused) && count !== undefined && count > 0)
      return { scope: one.scope, count };
  }
  return undefined;
}

/**
 * **The pick, until Asks closes** (decision 4): held in memory per project, never written to the
 * layout file, and Chat again once Asks closes (`forgetAsksScope`). Held outside the view so a
 * notification's landing can widen it to reach its chat (`landOnGroupIn`).
 */
const picks = new Map<string, AsksScope>();
const listeners = new Set<() => void>();
const told = () => {
  for (const listener of listeners) listener();
};
const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
};

/** The scope picked in `plane`'s Asks since it opened, or Chat. */
export function keptAsksScope(plane: string): AsksScope {
  return picks.get(plane) ?? "chat";
}

/** Picks `scope` in `plane`'s Asks, until it closes. */
export function pickAsksScope(plane: string, scope: AsksScope): void {
  if (keptAsksScope(plane) === scope) return;
  if (scope === "chat") picks.delete(plane);
  else picks.set(plane, scope);
  told();
}

/** `plane`'s Asks closed: it opens on Chat next time. */
export function forgetAsksScope(plane: string): void {
  if (!picks.delete(plane)) return;
  told();
}

/** {@link keptAsksScope}, for a component that redraws when it is picked again. */
export function useAsksScope(plane: string): AsksScope {
  return useSyncExternalStore(subscribe, () => keptAsksScope(plane));
}
