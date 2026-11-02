// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { commands } from "../bindings";
import type { Frame } from "../windowprefs";
import { drawIn, inForce, sameTheme } from "./theme";

/** How long, in milliseconds, after each failed ask the next is made. */
export const PARTS_ASKED_AGAIN_AFTER: readonly number[] = [250, 1000, 4000];

/** The parts the core measured under `ticket`, or `undefined` when it did not answer with them. */
async function asked(ticket: number): Promise<string | undefined> {
  try {
    const answered = await commands.firstFrameParts(ticket);
    return answered.status === "ok" ? answered.data : undefined;
  } catch {
    return undefined;
  }
}

/**
 * **Lands the parts of a first frame drawn without them** (#4410): a theme measured past the
 * window's creation (#4337). Asked as the window starts, so they land when the measure does,
 * however long the restore takes, and asked again when an ask fails. A frame that has its parts
 * asks nothing, so a shipped theme, measured in under a millisecond, costs a launch no ask.
 *
 * Only the frame itself is given its parts. A theme drawn in its place while this asks stays, and
 * nothing more is asked: the core has answered for that one with its own parts. Past the last
 * ask the frame stays as it is, and the window's ask of what is in force still lands them.
 */
export async function partsLand(frame: Frame): Promise<void> {
  const ticket = frame.parts_later;
  if (ticket === null) return;
  const stillDrawn = () => sameTheme(inForce(), frame.theme);
  for (const wait of [0, ...PARTS_ASKED_AGAIN_AFTER]) {
    if (wait > 0) await new Promise((waited) => setTimeout(waited, wait));
    if (!stillDrawn()) return;
    const parts = await asked(ticket);
    if (parts === undefined) continue;
    if (stillDrawn()) drawIn({ ...frame.theme, parts });
    return;
  }
}
