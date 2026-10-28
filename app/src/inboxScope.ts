// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";
import { keepFacet, keptFacet, onProjectViews, type Facet } from "./projectViews";

/**
 * **Which asks the Inbox lists** (qw359): every project's this window knows of, this project's
 * (the default), or those of the workspace in front on the strip. At the plane root the
 * workspace's are the asks of the chats filed in no workspace.
 *
 * Only what waits on the person is narrowed. A project's Notices and its updates are the
 * project's, and are listed whatever the scope.
 */
export type InboxScope = "all" | "project" | "workspace";

/** The scopes, as the switch says them, widest first. */
export const INBOX_SCOPES: readonly { scope: InboxScope; says: string; of: string }[] = [
  { scope: "all", says: "All projects", of: "all projects" },
  { scope: "project", says: "This project", of: "this project" },
  { scope: "workspace", says: "This workspace", of: "this workspace" },
];

/** The scope an Inbox is drawn in where the person picked none. */
export const DEFAULT_INBOX_SCOPE: InboxScope = "project";

const isScope = (held: unknown): held is InboxScope =>
  INBOX_SCOPES.some((one) => one.scope === held);

/**
 * **The scope the person last picked for `plane`**, or the project's.
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
 * **The scope kept as `facet`'s `scope` for `plane`**, or the project's: the Inbox's and the
 * Browsers grid's (#2027) are the one switch, each kept under its own facet.
 */
export function keptScopeOf(plane: string | undefined, facet: Facet): InboxScope {
  const held = keptFacet(plane, facet);
  if (held === null || typeof held !== "object" || Array.isArray(held)) return DEFAULT_INBOX_SCOPE;
  const scope = Object.prototype.hasOwnProperty.call(held, "scope")
    ? (held as Record<string, unknown>).scope
    : undefined;
  return isScope(scope) ? scope : DEFAULT_INBOX_SCOPE;
}

/** Keeps `scope` as `facet`'s `scope` for `plane`. The default is kept as nothing. */
export function keepScopeOf(plane: string | undefined, facet: Facet, scope: InboxScope): void {
  keepFacet(plane, facet, scope === DEFAULT_INBOX_SCOPE ? undefined : { scope });
}

/** How many asks each scope lists; a scope that is not offered has no count. */
export type ScopeCounts = Partial<Record<InboxScope, number>>;

/**
 * **The scope an Inbox is drawn in**: the one picked where it is offered, else the project's,
 * which every Inbox has. A window that names no workspace offers no workspace scope, so a pick
 * kept for one falls back and is not lost.
 */
export function scopeDrawn(picked: InboxScope, counts: ScopeCounts): InboxScope {
  return counts[picked] === undefined ? DEFAULT_INBOX_SCOPE : picked;
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
  for (const one of INBOX_SCOPES.slice(0, at).reverse()) {
    const count = counts[one.scope];
    if (count !== undefined && count > 0) return { scope: one.scope, count };
  }
  return undefined;
}

/** What `scope` is called in a sentence: "this workspace". */
export const scopeOf = (scope: InboxScope): string =>
  INBOX_SCOPES.find((one) => one.scope === scope)?.of ?? "";
