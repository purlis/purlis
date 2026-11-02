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

/** How long, in milliseconds, a frame that stands in for an extension's theme waits for the
 *  window to ask what is in force before it lands its own parts (#4722): long enough for a
 *  launch's restore to say which project is in front, so a window that asks never draws twice. */
export const STAND_IN_WAITS_FOR_AN_ASK = 5000;

/** The first frame standing in for an extension's theme, while it waits for the window's ask of
 *  what is in force to settle; `undefined` once it has, or when there is none. */
let standIn: { frame: Frame; never: ReturnType<typeof setTimeout> } | undefined;

/**
 * **Lands the parts of a first frame drawn without them** (#4410): a theme measured past the
 * window's creation (#4337). Asked as the window starts, so they land when the measure does,
 * however long the restore takes, and asked again when an ask fails. A frame that has its parts
 * asks nothing, so a shipped theme, measured in under a millisecond, costs a launch no ask.
 *
 * Only the frame itself is given its parts. A theme drawn in its place while this asks stays, and
 * nothing more is asked: the core has answered for that one with its own parts. Past the last
 * ask the frame stays as it is, and the window's ask of what is in force still lands them.
 *
 * **A frame that stands in for an extension's theme waits** (#4722): the window's ask of what is
 * in force draws that theme over it, so landing its parts first would change the window twice.
 * They land once that ask settles with the frame still drawn ({@link inForceSettled}): an answer
 * that failed, or that kept the window as it is. A window that has not asked within
 * {@link STAND_IN_WAITS_FOR_AN_ASK} lands them then.
 */
export async function partsLand(frame: Frame): Promise<void> {
  if (frame.parts_later === null) return;
  if (!frame.stands_in) return land(frame);
  if (standIn !== undefined) clearTimeout(standIn.never);
  standIn = { frame, never: setTimeout(() => void inForceSettled(), STAND_IN_WAITS_FOR_AN_ASK) };
}

/** The window asked what is in force: a stand-in waits for that answer, however long it takes. */
export function inForceAsked(): void {
  if (standIn !== undefined) clearTimeout(standIn.never);
}

/** The window's ask of what is in force settled, drawn or not: a stand-in still drawn lands its
 *  own parts, once. One drawn over asks nothing; the answer came with its own parts. */
export async function inForceSettled(): Promise<void> {
  const waiting = standIn;
  if (waiting === undefined) return;
  standIn = undefined;
  clearTimeout(waiting.never);
  await land(waiting.frame);
}

/** Asks for the parts of `frame`, which has a ticket, for as long as it is drawn. */
async function land(frame: Frame): Promise<void> {
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
