// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState, useSyncExternalStore } from "react";
import { commands, SPECIMEN_BUSY, type ThemeAt } from "../bindings";

/**
 * **The pictures on Settings › Theme's cards** (#2331): the theme specimen (#2330), drawn by the
 * app in a hidden window of its own for each card's theme in each of its appearances
 * (`theme_specimen`), and kept by what the theme draws (the card's `stamp`), so a card is
 * photographed again only when its theme changed.
 *
 * **Only the cards on screen are pictured** (#2614): a picture is asked for once its card is in
 * sight ({@link useOnScreen}), and one still waiting its turn when its card leaves the screen is
 * not asked for at all, so opening the tab with many themes paints the cards in view, and
 * scrolling paints the rest.
 *
 * **One at a time, in the order the cards asked**: the app draws one specimen at a time and
 * answers a second ask that finds one being drawn with a refusal, so the window queues its own
 * and never sends two at once. A chat's `theme_preview` goes first: the app has the picture
 * being drawn here give way to it, and answers this window's asks busy while a chat waits. One
 * refused because the painter was busy is asked again after a wait longer than a specimen's
 * budget, a few times. A specimen the app could not draw is said on the card in the app's words,
 * and is never kept: the card asks again the next time it is shown.
 *
 * **At most {@link MOST_KEPT} pictures are kept in all**: past that, the one shown least recently
 * is let go, and asked for again if its card comes back. One a card on screen shows, or one being
 * drawn, is never let go.
 */

/** A card's picture: a data URL, being drawn, or why there is none. */
export type Picture =
  { state: "drawing" } | { state: "drawn"; src: string } | { state: "none"; why: string };

/** Every picture kept, the one shown least recently first. */
const pictures = new Map<string, Picture>();
const listeners = new Set<() => void>();
/** The pictures asked for and not yet sent, in the order asked. */
const queue: { key: string; ask: () => Promise<Picture> }[] = [];
/** How many cards on screen show each picture: none of these is let go. */
const onScreen = new Map<string, number>();
/** The waits for a busy painter, each cleared when the pictures are forgotten. */
const waits = new Set<ReturnType<typeof setTimeout>>();
let drawing = false;
/** Which life of the pictures this is: an answer to an ask of an earlier one is dropped. */
let life = 0;

/** How long a busy painter is waited for before asking again: longer than one specimen's
 *  budget (`themepreview::BUDGET`, 5 s), which is the most a chat's ask holds it. */
export const BUSY_WAIT_MS = 6_000;

/** How many times a busy painter is asked in all. */
export const BUSY_TRIES = 3;

/** **How many pictures are kept in all** (#2614): two screens of cards with their pictures. A
 *  picture is a full specimen, a PNG of a few hundred kilobytes. */
export const MOST_KEPT = 48;

/** How far past the screen a card counts as on it, so a picture is ready as it scrolls in. */
const AHEAD = "200px 0px";

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

/** Keeps `picture` as the one shown most recently. */
function keep(key: string, picture: Picture): void {
  pictures.delete(key);
  pictures.set(key, picture);
}

/** Lets go of the pictures shown least recently, past {@link MOST_KEPT}, that no card on screen
 *  shows and none is drawing. */
function trim(): void {
  for (const [key, picture] of pictures) {
    if (pictures.size <= MOST_KEPT) return;
    if (picture.state === "drawing" || onScreen.has(key)) continue;
    pictures.delete(key);
  }
}

/** Draws the next picture asked for, once the last is done. */
function next() {
  if (drawing) return;
  const one = queue.shift();
  if (one === undefined) return;
  drawing = true;
  const asked = life;
  void one
    .ask()
    .then((picture) => {
      if (asked === life) keep(one.key, picture);
    })
    .finally(() => {
      // The pictures were forgotten while this was drawn: what is drawn now is another's.
      if (asked !== life) return;
      drawing = false;
      trim();
      told();
      next();
    });
}

/** Waits `ms`, a wait the pictures being forgotten clears. */
function waited(ms: number): Promise<void> {
  return new Promise((done) => {
    const wait = setTimeout(() => {
      waits.delete(wait);
      done();
    }, ms);
    waits.add(wait);
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
    await waited(BUSY_WAIT_MS);
  }
}

/** A card showing `key` came on screen. */
function shown(key: string): void {
  onScreen.set(key, (onScreen.get(key) ?? 0) + 1);
  const kept = pictures.get(key);
  if (kept !== undefined) keep(key, kept);
}

/** A card showing `key` left the screen: a picture no card on screen shows any more and still
 *  waiting its turn is not asked for. */
function unshown(key: string): void {
  const left = (onScreen.get(key) ?? 1) - 1;
  if (left > 0) {
    onScreen.set(key, left);
    return;
  }
  onScreen.delete(key);
  const waiting = queue.findIndex((one) => one.key === key);
  if (waiting >= 0) {
    queue.splice(waiting, 1);
    pictures.delete(key);
  }
  trim();
}

/** **The picture of `theme` in `appearance`**, asked for the first time a card shows it on
 *  screen (`seen`): of the theme folder of the level `from`, for a folder's card (#2516). */
export function useSpecimen(
  at: ThemeAt | null,
  theme: string,
  from: string | null,
  stamp: string,
  appearance: string,
  seen: boolean,
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
    if (!seen) return;
    const mine = life;
    shown(key);
    // A picture drawn or being drawn is kept; one that could not be drawn is asked again.
    const kept = pictures.get(key);
    if (kept === undefined || kept.state === "none") {
      keep(key, DRAWING);
      queue.push({ key, ask: () => asked(at, theme, from, appearance) });
      told();
      next();
    }
    return () => {
      // The pictures were forgotten since: there is nothing of this card's left to let go.
      if (mine === life) unshown(key);
    };
    // `key` holds every input the ask is made from.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, seen]);
  return picture ?? DRAWING;
}

/** What a card shows while its picture is drawn. */
const DRAWING: Picture = { state: "drawing" };

/** Every element watched for coming on screen, and what to tell when it does or leaves. */
const watched = new Map<Element, (on: boolean) => void>();
/** The one watcher of every card's pictures; `null` in a webview without one. */
let watcher: IntersectionObserver | null | undefined;

function watcherOf(): IntersectionObserver | null {
  if (watcher === undefined)
    watcher =
      typeof IntersectionObserver === "function"
        ? new IntersectionObserver(
            (entries) => {
              for (const one of entries) watched.get(one.target)?.(one.isIntersecting);
            },
            { rootMargin: AHEAD },
          )
        : null;
  return watcher;
}

/** **Whether the element given the ref is on screen**, or near enough to it to be pictured
 *  ahead: as `IntersectionObserver` tells it, nothing until it has; a webview without one shows
 *  every card as on screen. The ref is a callback, so an element rendered later (a refused card
 *  that becomes a theme while the tab is open) is watched once it is there. */
export function useOnScreen(): [(element: Element | null) => void, boolean] {
  const [element, setElement] = useState<Element | null>(null);
  const [on, setOn] = useState(() => typeof IntersectionObserver !== "function");
  useEffect(() => {
    const watching = watcherOf();
    if (element === null || watching === null) return;
    watched.set(element, setOn);
    watching.observe(element);
    return () => {
      watched.delete(element);
      watching.unobserve(element);
    };
  }, [element]);
  return [setElement, on];
}

/** For tests: how many pictures are kept, drawn or not. */
export function keptSpecimens(): number {
  return pictures.size;
}

/** For tests: forget every picture, every ask and every wait, and stop watching the screen, so
 *  nothing one test started reaches the next. */
export function forgetSpecimens(): void {
  life++;
  pictures.clear();
  queue.length = 0;
  onScreen.clear();
  for (const wait of waits) clearTimeout(wait);
  waits.clear();
  drawing = false;
  watcher?.disconnect();
  watcher = undefined;
  watched.clear();
}
