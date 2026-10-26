// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useLayoutEffect, useRef, useState, type RefObject } from "react";

/**
 * **Where each answer that grants is drawn on screen, and since when** (#3167): the place the
 * press-settle guard (`noticeSettle.ts`, the Inbox's rows) reads, so a press on an answer that
 * moved just now can wait.
 *
 * **The answer's own box**, never the box of the line it sits in: a line can keep its top while
 * the answer in it moves (its words wrap again at a new width and the answer goes left or down),
 * and the answer's own box is what the pointer lands on.
 *
 * **What counts as a move**: any change of the box's size, or of its place in what scrolls it
 * (its place on screen with every scroll above it added back), against where it last settled
 * (so a drift too small to see between two reads still adds up). **The person's own scroll is
 * no move**: one that follows, within {@link OWN_SCROLL_MS}, their wheel or trackpad, a key that
 * scrolls, a touch, or a press on a scrollbar (#3217): it moves nothing under them they did not
 * choose to move. **Any other scroll is one, read on screen**: a scroll the window made (a box
 * brought into view, a focus that scrolled, a scroll put back), and one the page was made to
 * take: a box that scrolls cannot stay where it was scrolled to once what it holds got shorter
 * than that (the browser clamps it). Everything in it slides on screen though nothing in it
 * moved inside it.
 *
 * **When it is read**: in the commit of each render of a watching component (its own answers
 * only), in the next frame after anything that can move one (a change under the watched root, a
 * size change of the answer or of any box around it, a scroll anywhere, the window's size, a
 * transition or an animation ending: one pass for every watch, however many asked), and again at
 * the press itself (its own answers), so a move no observer saw still holds the press. A pass
 * only reads, and nothing is written in between, so it costs at most the one layout the frame
 * needs anyway.
 */

/** Less than this, in CSS pixels, is no move: what rounding and a transform's subpixels say. */
const STILL = 0.5;

/** **How long after the person's own scroll input a scroll is still theirs** (#3217), in ms:
 *  long enough for a key's smooth scroll to end, and a trackpad's glide sends wheel events as
 *  it goes. */
export const OWN_SCROLL_MS = 500;

/** When the person last asked for a scroll, and whether they are holding a scrollbar now. */
let ownScroll = { at: -Infinity, holding: false };

/** **Whether a scroll now is the person's own**: it follows their scroll input closely. */
const scrollIsOwn = () => ownScroll.holding || performance.now() - ownScroll.at < OWN_SCROLL_MS;

/** The keys that scroll what has the keyboard, or bring the next stop into view. */
const SCROLL_KEYS = new Set([
  "ArrowUp",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "PageUp",
  "PageDown",
  "Home",
  "End",
  " ",
  "Tab",
]);

/** Whether `el` takes the keys it is given as words: they scroll nothing there (but Tab). */
const writesIn = (el: EventTarget | null) =>
  el instanceof HTMLElement &&
  (el.isContentEditable || el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement);

/** Whether `el` scrolls: a press on it is on its scrollbar, or on the box it scrolls. */
const scrolls = (el: EventTarget | null) =>
  el instanceof Element &&
  (el.scrollHeight > el.clientHeight + STILL || el.scrollWidth > el.clientWidth + STILL);

/** **The person's own scroll input**, heard before anything acts on it. */
function heard(event: Event) {
  const now = performance.now();
  switch (event.type) {
    case "wheel":
    case "touchstart":
    case "touchmove":
      ownScroll = { ...ownScroll, at: now };
      return;
    case "keydown": {
      const key = (event as KeyboardEvent).key;
      if (SCROLL_KEYS.has(key) && (key === "Tab" || !writesIn(event.target))) {
        ownScroll = { ...ownScroll, at: now };
      }
      return;
    }
    case "pointerdown":
      if (scrolls(event.target)) ownScroll = { at: now, holding: true };
      return;
    default:
      // A scrollbar let go, or a touch ended: its glide is still theirs for a moment.
      if (ownScroll.holding || event.type === "touchend") ownScroll = { at: now, holding: false };
  }
}

const HEARD = [
  "wheel",
  "touchstart",
  "touchmove",
  "touchend",
  "keydown",
  "pointerdown",
  "pointerup",
  "pointercancel",
] as const;

/** A box's place, and since when it is there. */
export type WayPlace = {
  /** The box on screen. */
  left: number;
  top: number;
  width: number;
  height: number;
  /** The box's place with every scroll above it added back: where it is in what scrolls it. */
  x: number;
  y: number;
  /** When it came to be where it is: `-Infinity` for a first drawing that is no move. */
  at: number;
};

/** One component's watch: its root, the answers it guards, and their places. */
type Watch = {
  root: () => Element | null;
  ways: (root: Element) => Iterable<Element>;
  firstMoves: (way: Element) => boolean;
  places: WeakMap<Element, WayPlace>;
  /** The name an answer's place is remembered by past this drawing, if it has one. */
  nameOf?: (way: Element) => string | undefined;
  /** Where each named answer was, kept past this drawing (#3167, D-3167-6). */
  remembered?: () => Map<string, WayPlace>;
  /** Whether a newly drawn answer may be read against its remembered place now: only on the
   *  read that is the redrawing itself (D-3167-10). */
  recalls?: () => boolean;
  /** Whether the root is drawn now: while it is not, its answers have no place to read, and
   *  nothing is read (#3302). */
  drawn?: (root: Element) => boolean;
  /** Where each box that scrolls one of its answers was scrolled to, at its last read. */
  scrolled: WeakMap<Element, Scrolled>;
  /** What changes under the root: read again in the next frame. */
  changes?: MutationObserver;
  /** The root `changes` watches: a root drawn later, or drawn again, is watched from then. */
  watched?: Element | null;
};

/** Where a box that scrolls was scrolled to, and how far it could be. */
type Scrolled = { top: number; left: number; maxTop: number; maxLeft: number };

const watches = new Set<Watch>();
/** Every box a size observer watches, so one that left the page is let go. */
const sized = new Set<Element>();
let sizes: ResizeObserver | undefined;
let frame: number | undefined;

const differ = (a: number, b: number) => Math.abs(a - b) > STILL;

/** What every box from one up to the page has scrolled, and whether one of them was made to. */
type Chain = { x: number; y: number; made: boolean };
/** One pass's chains, each box's read once however many answers it holds. */
type Pass = Map<Element, Chain> & { named?: Set<string> };
const PAGE: Chain = { x: 0, y: 0, made: false };
/** What a watch with nothing kept past its drawing keeps: nothing, as it is never named. */
const NOTHING_KEPT = new Map<string, WayPlace>();

/** **Whether `box` was made to scroll since the watch last read it**: it was scrolled further
 *  than what it holds now lets it be, so the browser clamped it; or it scrolled, and not as the
 *  person asked (#3217). */
function madeToScroll(watch: Watch, box: Element): boolean {
  const now: Scrolled = {
    top: box.scrollTop,
    left: box.scrollLeft,
    maxTop: box.scrollHeight - box.clientHeight,
    maxLeft: box.scrollWidth - box.clientWidth,
  };
  const was = watch.scrolled.get(box);
  watch.scrolled.set(box, now);
  if (was === undefined) return false;
  const clamped =
    (now.top < was.top - STILL && was.top > now.maxTop + STILL) ||
    (now.left < was.left - STILL && was.left > now.maxLeft + STILL);
  const scrolled = differ(now.top, was.top) || differ(now.left, was.left);
  return clamped || (scrolled && !scrollIsOwn());
}

/** Watches `el`'s size, once. */
function sizeOf(el: Element) {
  if (sizes === undefined || sized.has(el)) return;
  sized.add(el);
  sizes.observe(el);
}

/** What `el` and every box around it up to the page have scrolled, read once a pass. */
function chainOf(watch: Watch, el: Element | null, pass: Pass): Chain {
  if (el === null) return PAGE;
  const known = pass.get(el);
  if (known !== undefined) return known;
  const up = chainOf(watch, el.parentElement, pass);
  sizeOf(el);
  const made = madeToScroll(watch, el) || up.made;
  const chain = { x: up.x + el.scrollLeft, y: up.y + el.scrollTop, made };
  pass.set(el, chain);
  return chain;
}

/** Reads where `way` is drawn now, and keeps when it came to be there. */
function place(watch: Watch, way: Element, pass: Pass, at: number) {
  const box = way.getBoundingClientRect();
  sizeOf(way);
  // A box drawn nowhere (hidden, or not on the page) has no place a scroll could change.
  const drawn = box.width !== 0 || box.height !== 0;
  const around = drawn ? chainOf(watch, way.parentElement, pass) : PAGE;
  const x = box.left + around.x;
  const y = box.top + around.y;
  const made = around.made;
  // An answer drawn again where its place is remembered (its band drawn anew) is read against
  // where it was, not as a first drawing: on the redrawing's own read only, so a new answer
  // arriving later is never taken for one that stood (D-3167-10). Every read remembers.
  const name = watch.nameOf?.(way);
  const kept = name === undefined ? undefined : watch.remembered?.();
  const recalled = name !== undefined && watch.recalls?.() === true ? kept?.get(name) : undefined;
  const was = watch.places.get(way) ?? recalled;
  const moved =
    was === undefined
      ? watch.firstMoves(way)
      : differ(box.width, was.width) ||
        differ(box.height, was.height) ||
        differ(x, was.x) ||
        differ(y, was.y) ||
        (made && (differ(box.left, was.left) || differ(box.top, was.top)));
  // **Measured from where it last settled**, never from the last read: a box that drifts a little
  // less than `STILL` between reads still moves as far as it goes in all. Only the on-screen box
  // the clamp check compares is read again every time.
  const keep = !moved && was !== undefined;
  const now: WayPlace = {
    left: box.left,
    top: box.top,
    width: keep ? was.width : box.width,
    height: keep ? was.height : box.height,
    x: keep ? was.x : x,
    y: keep ? was.y : y,
    at: moved ? at : (was?.at ?? -Infinity),
  };
  watch.places.set(way, now);
  if (name !== undefined) {
    kept?.set(name, now);
    pass.named?.add(name);
  }
}

/** **One pass over one watch**: every answer it guards is read where it is drawn now. */
function look(watch: Watch) {
  const root = watch.root();
  watchRoot(watch, root);
  // **Read only once drawn** (#3302): a root not drawn yet (a band hidden while it holds no
  // Notice) would read every answer at no size, and the read after it, once drawn, would take
  // each for a move. Nothing is read, written or forgotten until it is drawn.
  if (root === null || watch.drawn?.(root) === false) return;
  const pass: Pass = new Map();
  pass.named = new Set();
  const at = performance.now();
  for (const way of watch.ways(root)) place(watch, way, pass, at);
  // What the root no longer draws is no longer remembered: the memory holds what stands now.
  const kept = watch.remembered?.();
  if (kept !== undefined)
    for (const name of kept.keys()) if (!pass.named.has(name)) kept.delete(name);
}

/** **One pass over every watch**, in the frame after anything that can move an answer. */
function lookAll() {
  for (const watch of watches) look(watch);
  // A box that left the page is no longer watched for its size.
  for (const el of sized) {
    if (!el.isConnected) {
      sized.delete(el);
      sizes?.unobserve(el);
    }
  }
}

/** Reads every place again in the next frame, once however many things asked. */
function inAFrame() {
  if (frame !== undefined) return;
  const next =
    typeof requestAnimationFrame === "function"
      ? requestAnimationFrame
      : (run: () => void) => setTimeout(run, 16) as unknown as number;
  frame = next(() => {
    frame = undefined;
    lookAll();
  });
}

const LISTENED = ["scroll", "transitionend", "animationend"] as const;

function listen(on: boolean) {
  const how = { capture: true, passive: true } as const;
  for (const kind of LISTENED) {
    if (on) document.addEventListener(kind, inAFrame, how);
    else document.removeEventListener(kind, inAFrame, how);
  }
  for (const kind of HEARD) {
    if (on) document.addEventListener(kind, heard, how);
    else document.removeEventListener(kind, heard, how);
  }
  if (!on) ownScroll = { at: -Infinity, holding: false };
  if (on) window.addEventListener("resize", inAFrame);
  else window.removeEventListener("resize", inAFrame);
  // A font that loads late lays the words out again.
  const fonts = (document as Document & { fonts?: EventTarget }).fonts;
  if (on) fonts?.addEventListener?.("loadingdone", inAFrame);
  else fonts?.removeEventListener?.("loadingdone", inAFrame);
}

function start(watch: Watch) {
  if (watches.size === 0) {
    sizes = typeof ResizeObserver === "undefined" ? undefined : new ResizeObserver(inAFrame);
    listen(true);
  }
  watches.add(watch);
  watchRoot(watch, watch.root());
}

/** Watches what changes under `root`, the watch's root now. */
function watchRoot(watch: Watch, root: Element | null) {
  if (watch.watched === root) return;
  watch.changes?.disconnect();
  watch.watched = root;
  if (root === null || typeof MutationObserver === "undefined") return;
  watch.changes ??= new MutationObserver(inAFrame);
  watch.changes.observe(root, {
    childList: true,
    subtree: true,
    characterData: true,
    attributes: true,
    // What lays a box out again: shown or hidden, a class, a style, opened or closed.
    attributeFilter: ["hidden", "class", "style", "open", "aria-expanded"],
  });
}

function stop(watch: Watch) {
  watch.changes?.disconnect();
  // Started again (as React's strict mode does), it watches its root again.
  watch.watched = undefined;
  watches.delete(watch);
  if (watches.size === 0) {
    listen(false);
    sizes?.disconnect();
    sizes = undefined;
    sized.clear();
    if (frame !== undefined && typeof cancelAnimationFrame === "function") {
      cancelAnimationFrame(frame);
    }
    frame = undefined;
  }
}

/**
 * **Watches where the answers under `at` are drawn** (#3167): `ways` lists the answers that
 * grant under the root, and `firstMoves` says whether an answer's first drawing counts as a move.
 * `movedAt` reads every place again and says when the answer `way` (or, without one, the last of
 * them to move) came to be where it is now.
 */
export function useWayPlaces<T extends Element>(
  at: RefObject<T | null>,
  ways: (root: T) => Iterable<Element>,
  firstMoves: (way: Element) => boolean,
  past?: {
    nameOf: (way: Element) => string | undefined;
    remembered: () => Map<string, WayPlace>;
    recalls: () => boolean;
  },
  drawn?: (root: T) => boolean,
) {
  // The callbacks of the last render, which the watch asks when it reads.
  const said = useRef({ ways, firstMoves, past, drawn });
  const [watch] = useState<Watch>(() => ({
    root: () => at.current,
    ways: (root) => said.current.ways(root as T),
    firstMoves: (way) => said.current.firstMoves(way),
    nameOf: (way) => said.current.past?.nameOf(way),
    remembered: () => said.current.past?.remembered() ?? NOTHING_KEPT,
    recalls: () => said.current.past?.recalls() === true,
    drawn: (root) => said.current.drawn?.(root as T) !== false,
    places: new WeakMap(),
    scrolled: new WeakMap(),
  }));
  useLayoutEffect(() => {
    start(watch);
    return () => stop(watch);
  }, [watch]);
  // After every render, in the commit itself: what it drew is read where it is drawn before
  // anything else can move it, so a move right after its first drawing is a move.
  useLayoutEffect(() => {
    said.current = { ways, firstMoves, past, drawn };
    look(watch);
  });
  const movedAt = (way?: Element): number => {
    look(watch);
    if (way !== undefined) {
      if (!watch.places.has(way)) place(watch, way, new Map(), performance.now());
      return watch.places.get(way)?.at ?? performance.now();
    }
    const root = at.current;
    let last = -Infinity;
    if (root !== null) {
      for (const each of watch.ways(root)) {
        last = Math.max(last, watch.places.get(each)?.at ?? last);
      }
    }
    return last;
  };
  return { movedAt };
}
