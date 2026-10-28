// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useMemo, useState } from "react";

/**
 * **Who has worked in each workspace, as this window saw it** (#3566): the personas each
 * workspace's chats ran as, kept after the chat closes. With the personas its session records
 * name, a workspace's Personas view draws them under In this workspace (#3483).
 *
 * The chats listed now are not enough on their own: most chats end with no session record, so
 * a persona whose chat closed would move from In this workspace back to On the plane, and back
 * again with its next chat. This keeps every persona the window has seen at work in a
 * workspace for as long as the window holds the project, and lets go of them all when it holds
 * another.
 */
export type WorkedHere = ReadonlyMap<string, ReadonlySet<string>>;

/** What a listed chat says of who worked where: its workspace, and its persona or none. */
export type WorkedRow = { readonly workspace: string; readonly persona: string | null };

const NONE: WorkedHere = new Map();

/**
 * `was`, with each persona `rows` names in its row's workspace. **`was` itself where they add
 * nothing**, so a list redrawn with no new persona in it redraws nothing that reads this.
 */
export function withChatPersonas(was: WorkedHere, rows: readonly WorkedRow[]): WorkedHere {
  let grown: Map<string, Set<string>> | undefined;
  for (const { workspace, persona } of rows) {
    if (persona === null) continue;
    if ((grown ?? was).get(workspace)?.has(persona) === true) continue;
    grown ??= new Map([...was].map(([at, personas]) => [at, new Set(personas)]));
    grown.set(workspace, (grown.get(workspace) ?? new Set()).add(persona));
  }
  return grown ?? was;
}

/**
 * {@link WorkedHere} for `project`, grown from the chats `rows` lists now. Nothing is kept from
 * another project: a window that moves to another starts it afresh.
 */
export function useWorkedHere(project: string | undefined, rows: readonly WorkedRow[]): WorkedHere {
  const [seen, setSeen] = useState<{ project: string | undefined; of: WorkedHere }>({
    project,
    of: NONE,
  });
  const was = seen.project === project ? seen.of : NONE;
  const now = useMemo(() => withChatPersonas(was, rows), [was, rows]);
  // Adjusted while drawing, as a list's held rows are: no draw shows the chats' personas
  // without the ones seen before.
  if (now !== seen.of || seen.project !== project) setSeen({ project, of: now });
  return now;
}
