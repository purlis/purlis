// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ChatRow } from "./chatsTree";
import { forgetProjectViews, keepFacet, keptFacet } from "./projectViews";
import { onLayoutMovedAside } from "./windowprefs";

/**
 * **The Chats list follows the focused workspace** (#1655, D-qw97-1): the rows of the trees
 * that started there, each with everything below it.
 *
 * A tree belongs to where its top row works: the chat a person opened, or a handoff, which
 * moved the work to where it now runs. A task stays under the chat that asked for it wherever
 * it works, so a tree is never cut in two (#1447). A chat started at the plane root belongs to
 * the root's view, by the word its rows say for it (D-qw97-2).
 *
 * The rows come back as they are where nothing is left out, so a list drawn from them is not
 * drawn again for nothing; otherwise the top rows are counted again among themselves, as a
 * screen reader reads them.
 */
export function inScope(rows: readonly ChatRow[], here: string): readonly ChatRow[] {
  const kept: ChatRow[] = [];
  let keeping = false;
  for (const row of rows) {
    if (row.level === 1) keeping = row.workspace === here;
    if (keeping) kept.push(row);
  }
  if (kept.length === rows.length) return rows;
  const tops = kept.filter((row) => row.level === 1).length;
  let at = 0;
  return kept.map((row) =>
    row.level === 1 ? { ...row, posinset: (at += 1), setsize: tops } : row,
  );
}

/**
 * **Which chats the Chats view lists** (#1679, B-15): the focused workspace's (the default,
 * #1655), or every workspace's. Whichever it is, a chat it leaves out that needs the person is
 * named at the top, with a way to it. **This tab went with the switch** (#4758): a chat isn't
 * a tab, so a tab holds no chats of its own to list, and a stored "tab" reads as Workspace.
 */
export type Scope = "workspace" | "all";

/** The scopes, as the switch says them, narrowest first. */
export const SCOPES: readonly { scope: Scope; says: string }[] = [
  { scope: "workspace", says: "Workspace" },
  { scope: "all", says: "All" },
];

/** Where web storage kept the pick before the layout file did (#1679). Read until the window
 *  moves it into the file ({@link settleScope}), and by nothing after. */
const LEGACY_KEY = "purlis.chats.scope:";

const isScope = (held: unknown): held is Scope => SCOPES.some((one) => one.scope === held);

/** The parts of the Chats view's entry for a project (`projectViews.ts`): its scope, and the
 *  folds set by hand (`chatFolds.ts`, #1687). */
export type ChatsPart = "scope" | "folds";

/** The Chats view's entry for `plane` as kept, read with own properties only: a value that is
 *  not an object is none. Untrusted: each part's owner checks it. */
export function keptChatsPart(plane: string, part: ChatsPart): unknown {
  const held = keptFacet(plane, "chats");
  if (held === null || typeof held !== "object" || Array.isArray(held)) return undefined;
  return Object.prototype.hasOwnProperty.call(held, part)
    ? (held as Record<string, unknown>)[part]
    : undefined;
}

/** Keeps `value` as `part` of the Chats view's entry for `plane`, beside the other part as it
 *  was kept; `undefined` takes the part out, and the entry goes with its last part. */
export function keepChatsPart(plane: string, part: ChatsPart, value: unknown): void {
  const next: Partial<Record<ChatsPart, unknown>> = {};
  for (const one of ["scope", "folds"] as const) {
    const kept = one === part ? value : keptChatsPart(plane, one);
    if (kept !== undefined) next[one] = kept;
  }
  keepFacet(plane, "chats", Object.keys(next).length === 0 ? undefined : next);
}

/** The scope the file (or this launch) keeps for `plane`, or nothing. */
function filed(plane: string): Scope | undefined {
  const scope = keptChatsPart(plane, "scope");
  return isScope(scope) ? scope : undefined;
}

/** What web storage kept for `plane` before the file did, where it kept a scope. */
function legacy(plane: string): Scope | undefined {
  try {
    const held = globalThis.localStorage.getItem(LEGACY_KEY + plane);
    return isScope(held) ? held : undefined;
  } catch {
    return undefined;
  }
}

/**
 * **The scope the person last picked for `plane`**, or the workspace's (#1679).
 *
 * Kept per project on this machine in `layout.json` v2 (B-11, #1696), under the project's
 * `chats.scope` (`projectViews.ts`): a view's scope is how this person looks at this project. A
 * pick web storage kept before that is read until {@link settleScope} moves it. A value that
 * is not a scope is the default.
 */
export function keptScope(plane: string | undefined): Scope {
  if (plane === undefined) return "workspace";
  return filed(plane) ?? legacy(plane) ?? "workspace";
}

/** Keeps `scope` as the one the person picked for `plane`. The default is kept as nothing. */
export function keepScope(plane: string | undefined, scope: Scope): void {
  if (plane === undefined) return;
  keepChatsPart(plane, "scope", scope === "workspace" ? undefined : scope);
  forgetLegacy(plane);
}

/**
 * **Moves the pick web storage kept for `plane` into the layout file, once** (#1696), as
 * `regions.ts` once moved its own key: the file's pick wins where it has one, and the key goes
 * either way, so the two never answer the same question.
 */
export function settleScope(plane: string | undefined): void {
  if (plane === undefined) return;
  const held = legacy(plane);
  if (held !== undefined && filed(plane) === undefined) keepScope(plane, held);
  forgetLegacy(plane);
}

function forgetLegacy(plane: string): void {
  try {
    globalThis.localStorage.removeItem(LEGACY_KEY + plane);
  } catch {
    // No storage: nothing kept there.
  }
}

/** Forgets every project's kept scope, as a machine that never picked one. For tests. */
export function forgetKeptScopes(): void {
  forgetProjectViews();
  forgetEveryLegacy();
}

function forgetEveryLegacy(): void {
  try {
    const keys = Object.keys(globalThis.localStorage).filter((key) => key.startsWith(LEGACY_KEY));
    for (const key of keys) globalThis.localStorage.removeItem(key);
  } catch {
    // No storage: nothing kept.
  }
}

// Use the default layout: every project's scope is the default, web storage's old picks too.
onLayoutMovedAside(forgetEveryLegacy);
