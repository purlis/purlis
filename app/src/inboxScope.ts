// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";
import { keepFacet, keptFacet, onProjectViews, type Facet } from "./projectViews";

/**
 * **The shared scopes** (qw359): the workspace in front on the strip, this project, or every
 * project this window knows of. The Browsers grid and Updates' scope switch read them; Asks has
 * its own, Chat, Workspace and Project (`asksScope.ts`), so its changes never move these. At the
 * plane root the workspace's are what the chats filed in no workspace hold.
 */
export type InboxScope = "all" | "project" | "workspace";

/**
 * The scopes, as the switch says them, narrowest first, in the words Chats and Asks use (ADR
 * 0121, amended 2026-11-12, #6347): Workspace, Project, and the grid's own All.
 */
export const INBOX_SCOPES: readonly { scope: InboxScope; says: string; of: string }[] = [
  { scope: "workspace", says: "Workspace", of: "this workspace" },
  { scope: "project", says: "Project", of: "this project" },
  { scope: "all", says: "All", of: "all projects" },
];

/**
 * **The scope kept under `inbox.scope` where the person picked none**: every project's, as it
 * was when Asks read it too (ADR 0121 decision 4). Updates, which reads it now, offers no All,
 * so it is drawn there as Project ({@link scopeDrawn}); a pick kept before reads back unchanged.
 */
export const DEFAULT_INBOX_SCOPE: InboxScope = "all";

/**
 * **The scope every list has**: the project's. What a list is drawn in where the scope picked is
 * not one it offers (Updates has no every project's), and the Browsers grid's default (#2027).
 */
export const PROJECT_SCOPE: InboxScope = "project";

/** Each facet's scope where the person picked none. */
const DEFAULT_OF: Partial<Record<Facet, InboxScope>> = { inbox: DEFAULT_INBOX_SCOPE };
const defaultOf = (facet: Facet): InboxScope => DEFAULT_OF[facet] ?? PROJECT_SCOPE;

const isScope = (held: unknown): held is InboxScope =>
  INBOX_SCOPES.some((one) => one.scope === held);

/**
 * **The scope the person last picked for `plane`**, or every project's.
 *
 * Kept where the Chats view keeps its own (`chatsScope.ts`): per project on this machine, in
 * `layout.json` v2 under the project's `inbox.scope` (`projectViews.ts`). It is how this person
 * looks at this project, never the project's own setting. What the file holds is untrusted:
 * read with own properties only, and a value that is not a scope is the default.
 */
export function keptInboxScope(plane: string | undefined): InboxScope {
  return keptScopeOf(plane, "inbox");
}

/** Keeps `scope` as the one the person picked for `plane`. The default is kept as nothing. */
export function keepInboxScope(plane: string | undefined, scope: InboxScope): void {
  keepScopeOf(plane, "inbox", scope);
}

/** {@link keptInboxScope}, for a component that redraws when it is picked again. */
export function useInboxScope(plane: string | undefined): InboxScope {
  return useSyncExternalStore(onProjectViews, () => keptInboxScope(plane));
}

/**
 * **The scope kept as `facet`'s `scope` for `plane`**, or that facet's default: the Inbox's
 * and the Browsers grid's (#2027) are the one switch, each kept under its own facet. The Inbox's
 * default is every project's; the grid's is the project's.
 */
export function keptScopeOf(plane: string | undefined, facet: Facet): InboxScope {
  const held = keptFacet(plane, facet);
  if (held === null || typeof held !== "object" || Array.isArray(held)) return defaultOf(facet);
  const scope = Object.prototype.hasOwnProperty.call(held, "scope")
    ? (held as Record<string, unknown>).scope
    : undefined;
  return isScope(scope) ? scope : defaultOf(facet);
}

/** Keeps `scope` as `facet`'s `scope` for `plane`. Its default is kept as nothing. */
export function keepScopeOf(plane: string | undefined, facet: Facet, scope: InboxScope): void {
  keepFacet(plane, facet, scope === defaultOf(facet) ? undefined : { scope });
}

/** How many asks each scope lists; a scope that is not offered has no count. */
export type ScopeCounts = Partial<Record<InboxScope, number>>;

/**
 * **The scope an Inbox is drawn in**: the one picked where it is offered, else the project's,
 * which every Inbox has. A window that names no workspace offers no workspace scope, and
 * Updates no every project's, so a pick kept for one falls back and is not lost.
 */
export function scopeDrawn(picked: InboxScope, counts: ScopeCounts): InboxScope {
  return counts[picked] === undefined ? PROJECT_SCOPE : picked;
}

/**
 * **The nearest wider scope with something waiting**, for an empty scope's way out: the
 * project's before every project's. Nothing where no wider scope lists anything.
 */
export function widerWaiting(
  scope: InboxScope,
  counts: ScopeCounts,
): { scope: InboxScope; count: number } | undefined {
  const at = INBOX_SCOPES.findIndex((one) => one.scope === scope);
  for (const one of INBOX_SCOPES.slice(at + 1)) {
    const count = counts[one.scope];
    if (count !== undefined && count > 0) return { scope: one.scope, count };
  }
  return undefined;
}

/** What `scope` is called in a sentence: "this workspace". */
export const scopeOf = (scope: InboxScope): string =>
  INBOX_SCOPES.find((one) => one.scope === scope)?.of ?? "";
