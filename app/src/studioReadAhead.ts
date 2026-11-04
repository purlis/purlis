// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { commands } from "./bindings";

/**
 * **The studio's skills, asked for as it is opened** (#4351, L10). Reading a project's skills is
 * most of an opening (some 280 ms with two hundred of them), and the tab used to ask only once it
 * was drawn, so the read waited for the draw. Opening the studio now asks at once, and the tab
 * takes that answer for the read it would have asked for: the two run side by side.
 *
 * Only an opening's read is taken. A tab reads afresh for anything that happens once it is open (a
 * skill changed on disk, a save, another scope picked), so it never draws an answer older than
 * what it was told. A read nobody takes is dropped once it is no longer fresh: one asked for a tab
 * that was already open, or an opening that drew nothing.
 */

type StudioAnswer = Awaited<ReturnType<typeof commands.skillStudio>>;

/** How long a read asked as the studio opened stands for the tab's own: far longer than a tab
 *  takes to draw, far shorter than anything a person does in it. */
export const READ_AHEAD_FRESH_MS = 2_000;

/** The last read asked as the studio opened, by whose skills it is. */
let ahead: { key: string; at: number; answer: Promise<StudioAnswer> } | undefined;

const keyOf = (plane: string, workspace: string | null, persona: string | null) =>
  JSON.stringify([plane, workspace, persona]);

/** **Asks for a scope's skills as the studio is opened on it**, for its tab to take. */
export function readStudioAhead(
  plane: string,
  workspace: string | null,
  persona: string | null,
): void {
  ahead = {
    key: keyOf(plane, workspace, persona),
    at: performance.now(),
    answer: commands.skillStudio(plane, workspace, persona),
  };
}

/**
 * **The studio tab's read as it opens**: the one asked as it was opened when there is a fresh one
 * for the same skills, or a read of its own. Asked twice for the same opening (React's strict
 * mode draws a tab twice) it answers the same read.
 */
export function studioOpeningRead(
  plane: string,
  workspace: string | null,
  persona: string | null,
): Promise<StudioAnswer> {
  const taken = ahead;
  if (
    taken !== undefined &&
    taken.key === keyOf(plane, workspace, persona) &&
    performance.now() - taken.at < READ_AHEAD_FRESH_MS
  )
    return taken.answer;
  return commands.skillStudio(plane, workspace, persona);
}
