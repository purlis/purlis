// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { commands, type PlaneId } from "./bindings";
import { handedSaid, handReference, referenceSaid, type Referenced } from "./references";
import type { Ran } from "./actions";

/**
 * **Several files or folders into a chat, as one paste** (#1151): the rows picked in the
 * explorer, handed to chat `session` of project `plane` (named `chat` in what is said) through
 * the core's `references_into_chat`, which types them in the chat's syntax, a space between
 * them, and never sends. One row goes the one-row way (`handReference`).
 *
 * The window names only the branch and the paths; the core places each one. Rows of two
 * branches, or of another project, are refused here without asking: one paste is one branch's.
 */
export async function handReferences(
  plane: PlaneId,
  session: number,
  chat: string,
  refs: readonly Referenced[],
): Promise<Ran> {
  const [first, ...rest] = refs;
  if (first === undefined) return { ok: false, refused: `Nothing was picked for ${chat}.` };
  if (rest.length === 0) return handReference(plane, session, chat, first);
  if (refs.some((one) => one.plane !== plane))
    return {
      ok: false,
      refused: `The picked rows are in another project, so nothing was handed to ${chat}.`,
    };
  if (
    rest.some(
      (one) =>
        one.workspace !== first.workspace || one.repo !== first.repo || one.piece !== first.piece,
    )
  )
    return {
      ok: false,
      refused: `The picked rows are in more than one branch, so nothing was handed to ${chat}.`,
    };
  const answer = await commands
    .referencesIntoChat(
      plane,
      first.workspace,
      first.repo,
      first.piece,
      refs.map((one) => one.path),
      session,
    )
    .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
  if (answer.status === "error") return { ok: false, refused: answer.error };
  return { ok: true, said: handedSaid(answer.data, chat) };
}

/** How the picked rows read in a title: one by its path, several by how many. */
export function picksSaid(refs: readonly Referenced[]): string {
  const [first, ...rest] = refs;
  if (first !== undefined && rest.length === 0) return referenceSaid(first);
  return `${refs.length} picked rows`;
}
