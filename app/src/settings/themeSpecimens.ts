// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useSyncExternalStore } from "react";
import { commands, SPECIMEN_BUSY, type ThemeAt } from "../bindings";

/**
 * **The pictures on Settings › Theme's cards** (#2331): the theme specimen (#2330), drawn by the
 * app in a hidden window of its own for each card's theme in each of its appearances
 * (`theme_specimen`), and kept for the life of the window by what the theme draws (the card's
 * `stamp`), so a card is photographed again only when its theme changed.
 *
 * **One at a time, in the order the cards asked**: the app draws one specimen at a time and
 * answers a second ask that finds one being drawn with a refusal, so the window queues its own
 * and never sends two at once. One refused because the painter was busy (a chat's
 * `theme_preview`, which holds it for up to its budget) is asked again after a wait longer than
 * that, a few times. A specimen the app could not draw is said on the card in the app's words,
 * and is never kept: the card asks again the next time it is shown.
 */

/** A card's picture: a data URL, being drawn, or why there is none. */
export type Picture =
  { state: "drawing" } | { state: "drawn"; src: string } | { state: "none"; why: string };

const pictures = new Map<string, Picture>();
const listeners = new Set<() => void>();
const queue: { key: string; ask: () => Promise<Picture> }[] = [];
let drawing = false;

/** How long a busy painter is waited for before asking again: longer than one specimen's
 *  budget (`themepreview::BUDGET`, 5 s), which is the most a chat's ask holds it. */
export const BUSY_WAIT_MS = 6_000;

/** How many times a busy painter is asked in all. */
export const BUSY_TRIES = 3;

function told() {
  for (const listener of listeners) listener();
}

/** The key a picture is kept by. */
export function pictureKey(
  at: ThemeAt | null,
  theme: string,
  from: string | null,
  stamp: string,
  appearance: string,
): string {
  return [at?.plane ?? "", at?.workspace ?? "", theme, from ?? "", stamp, appearance].join(
    "\u0000",
  );
}

/** Draws the next picture asked for, once the last is done. */
function next() {
  if (drawing) return;
  const one = queue.shift();
  if (one === undefined) return;
  drawing = true;
  void one
    .ask()
    .then((picture) => {
      pictures.set(one.key, picture);
    })
    .finally(() => {
      drawing = false;
      told();
      next();
    });
}

/** Asks the app for the specimen of `theme` (of the folder level `from`) in `appearance`, again
 *  after a wait while the painter is busy, {@link BUSY_TRIES} times at most. */
async function asked(
  at: ThemeAt | null,
  theme: string,
  from: string | null,
  appearance: string,
): Promise<Picture> {
  for (let tries = 1; ; tries++) {
    try {
      const answer = await commands.themeSpecimen(theme, appearance, at, from);
      if (answer.status === "ok")
        return { state: "drawn", src: `data:image/png;base64,${answer.data.png}` };
      if (answer.error !== SPECIMEN_BUSY || tries >= BUSY_TRIES)
        return { state: "none", why: answer.error };
    } catch (why) {
      return { state: "none", why: String(why) };
    }
    await new Promise((done) => setTimeout(done, BUSY_WAIT_MS));
  }
}

/** **The picture of `theme` in `appearance`**, asked for the first time a card shows it: of the
 *  theme folder of the level `from`, for a folder's card (#2516). */
export function useSpecimen(
  at: ThemeAt | null,
  theme: string,
  from: string | null,
  stamp: string,
  appearance: string,
): Picture {
  const key = pictureKey(at, theme, from, stamp, appearance);
  const picture = useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => pictures.get(key),
  );
  useEffect(() => {
    // A picture drawn or being drawn is kept; one that could not be drawn is asked again.
    const kept = pictures.get(key);
    if (kept !== undefined && kept.state !== "none") return;
    pictures.set(key, DRAWING);
    queue.push({ key, ask: () => asked(at, theme, from, appearance) });
    told();
    next();
    // `key` holds every input the ask is made from.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  return picture ?? DRAWING;
}

/** What a card shows while its picture is drawn. */
const DRAWING: Picture = { state: "drawing" };

/** For tests: forget every picture and every ask. */
export function forgetSpecimens(): void {
  pictures.clear();
  queue.length = 0;
  drawing = false;
}
