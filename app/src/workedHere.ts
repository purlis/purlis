// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useMemo, useState } from "react";
import { commands } from "./bindings";

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
 *
 * **And what the reopen record kept** (#3593): the host keeps each persona chat's workspace in
 * this clone's record as the chat runs (`purlis_core::reopen::worked`), and the window reads it
 * once as it takes the project, so a persona whose chat closed before a relaunch is still here
 * after it.
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

/** Who the reopen record says worked where, as rows; none where it cannot be asked. */
export type AskRecorded = (project: string) => Promise<readonly WorkedRow[]>;

const askTheRecord: AskRecorded = async (project) => {
  const said = await commands.workedHere(project);
  // Held to the shape, as every answer the window draws from is: a list of a workspace and its
  // personas, and anything else is nobody.
  if (said.status !== "ok" || !Array.isArray(said.data)) return [];
  return said.data.flatMap((one) =>
    typeof one?.workspace === "string" && Array.isArray(one.personas)
      ? one.personas
          .filter((persona): persona is string => typeof persona === "string")
          .map((persona) => ({ workspace: one.workspace, persona }))
      : [],
  );
};

/**
 * {@link WorkedHere} for `project`: what its reopen record kept (`ask`), grown from the chats
 * `rows` lists now. Nothing is kept from another project: a window that moves to another starts
 * it afresh, from that project's record.
 */
export function useWorkedHere(
  project: string | undefined,
  rows: readonly WorkedRow[],
  ask: AskRecorded = askTheRecord,
): WorkedHere {
  const [seen, setSeen] = useState<{ project: string | undefined; of: WorkedHere }>({
    project,
    of: NONE,
  });
  const was = seen.project === project ? seen.of : NONE;
  const now = useMemo(() => withChatPersonas(was, rows), [was, rows]);
  // Adjusted while drawing, as a list's held rows are: no draw shows the chats' personas
  // without the ones seen before.
  if (now !== seen.of || seen.project !== project) setSeen({ project, of: now });
  // Read once as the window takes the project: the chats it sees from then on are `rows`.
  useEffect(() => {
    if (project === undefined) return;
    let gone = false;
    ask(project)
      .then((recorded) => {
        if (gone) return;
        setSeen((held) =>
          held.project === project ? { project, of: withChatPersonas(held.of, recorded) } : held,
        );
      })
      // A record that cannot be read keeps what the window saw: nothing more is known.
      .catch(() => undefined);
    return () => {
      gone = true;
    };
  }, [ask, project]);
  return now;
}
