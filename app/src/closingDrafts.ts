// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { commands, PlaneId, WindowDraft } from "./bindings";
import { inTheWindow } from "./personaHostsAllow";
import { WINDOW_ONLY_SAID } from "./uiRpcLink";

/** The calls a close makes about its chats' theme drafts, on the window's client or a link's. */
export type DraftsClient = Pick<
  typeof commands,
  "themeDraftsOfChat" | "themeDraftKeep" | "themeDraftRevert" | "handDraftsCloseToTheWindow"
>;

/** What a link is told of its close, whatever its chats have: the close is the window's. */
export const CLOSE_IS_THE_WINDOWS =
  "Closing a chat is done in purlis's own window on this machine. Where a chat this closes has theme drafts, that window asks Keep or Revert about them.";

/** Why a close is not asked: purlis could not tell whether a chat has a theme draft. */
const COULD_NOT_TELL = (why: string) =>
  `purlis could not tell whether this chat has a theme draft, so it is not closed: ${why}`;

/**
 * **The theme drafts of every chat a close ends** (#2332), which it asks Keep or Revert about.
 * Where purlis cannot tell, the close does not go on unasked: it is refused, and says why. **On a
 * link nothing is asked** (#2708): closing a chat, and its drafts' Keep and Revert, are the
 * window's alone. **The close is handed to the window** (#2761), which asks the person about a
 * close of chats with drafts there; the link is told the same whatever its chats have
 * (`handed`), and where the hand fails, the words of any window-only command.
 */
export async function draftsOfClosing(
  client: DraftsClient,
  plane: PlaneId,
  ending: readonly number[],
): Promise<
  | { ok: true; drafts: WindowDraft[] }
  | { ok: false; refused: string }
  | { ok: false; handed: string }
> {
  if (!inTheWindow(client, "themeDraftsOfChat")) {
    const handed = await client
      .handDraftsCloseToTheWindow(plane, [...ending])
      .catch(() => ({ status: "error" as const, error: "" }));
    return handed.status === "ok"
      ? { ok: false, handed: CLOSE_IS_THE_WINDOWS }
      : { ok: false, refused: WINDOW_ONLY_SAID };
  }
  const asked = await Promise.all(
    ending.map((session) =>
      client
        .themeDraftsOfChat(plane, session)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) })),
    ),
  );
  const drafts: WindowDraft[] = [];
  for (const one of asked) {
    // An answer the core refused, or a call that failed, holds the close. An answer that is no
    // list is a core with no drafts to ask about (an older one, a test's).
    if (one.status === "error") return { ok: false, refused: COULD_NOT_TELL(one.error) };
    if (Array.isArray(one.data)) drafts.push(...one.data);
  }
  return { ok: true, drafts };
}

/** A theme kept as its chat closed: Keep then asks who draws it. */
export type KeptOnClose = { folder: string; workspace: string | null; keptTo: string };

/**
 * **Keeps or reverts a closing chat's theme drafts** (#2332), as its close was answered, one by
 * one. Answers each theme kept, **as it is kept** (#2708), so a later draft the core refuses does
 * not lose the question of who draws the ones already kept; and the refusal, which holds the
 * close.
 */
export async function settleClosingDrafts(
  client: DraftsClient,
  plane: PlaneId,
  drafts: readonly WindowDraft[],
  keep: boolean,
): Promise<{ kept: KeptOnClose[]; refused?: string }> {
  const kept: KeptOnClose[] = [];
  for (const one of drafts) {
    const done = keep
      ? // The close row said what Keep puts where, and what it replaces: the answer is the
        // person's yes to it.
        await client.themeDraftKeep(
          plane,
          one.session,
          one.folder,
          one.kept_to,
          one.replaces,
          one.changed,
        )
      : await client.themeDraftRevert(plane, one.session, one.folder);
    if (done.status === "error") return { kept, refused: done.error };
    if (keep) kept.push({ folder: one.folder, workspace: one.workspace, keptTo: one.kept_to });
  }
  return { kept };
}
