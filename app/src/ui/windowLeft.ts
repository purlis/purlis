// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useLayoutEffect, useRef } from "react";

/**
 * **Whether the pointer left the window, or the window the front** (#3095): the one answer, the
 * same in WebKit and Chromium, for whatever draws something only while the pointer is here (the
 * rail's edge peek, a hover card, a tooltip, a hold).
 *
 * **How an engine says the pointer left differs.** Chromium says `pointerleave` on the root;
 * WebKit may not, and says only a `pointerout` or `mouseout` into nothing (no related target).
 * All three are heard, and one trip out is told once: the next is armed by the first move back.
 * An element's own `pointerleave` is no substitute: the engine that misses the root's misses it.
 *
 * `"focus"` is told apart, as what is right for one is not always right for the other: a hold the
 * keyboard has outlives the pointer's going, and a tooltip the keyboard raised does too.
 *
 * One set of listeners serves every watcher, so a long list of rows costs one each.
 */

/** Why: the pointer left the window, or the window lost the focus. */
export type WindowLeft = "pointer" | "focus";
type Watcher = (why: WindowLeft) => void;

const watchers = new Set<Watcher>();
/** The pointer's trip out has been told, and the first move back has not come. */
let told = false;

function tell(why: WindowLeft) {
  for (const watcher of [...watchers]) {
    // Each on its own: one that throws takes no other's answer away, and is still reported.
    try {
      watcher(why);
    } catch (err: unknown) {
      queueMicrotask(() => {
        throw err;
      });
    }
  }
}

const pointerGone = () => {
  if (told) return;
  told = true;
  tell("pointer");
};
/** An out into nothing heard, and nothing yet to say the pointer is still here. */
let outHeard = false;
const outIntoNothing = (event: MouseEvent) => {
  if (event.relatedTarget !== null || outHeard) return;
  outHeard = true;
  // **Weighed once what was dispatched with it has been heard**: an out into nothing with an
  // over right behind it (an element taken away under the pointer, a driven move) is a pointer
  // that never left.
  queueMicrotask(() => {
    if (!outHeard) return;
    outHeard = false;
    pointerGone();
  });
};
const pointerOver = () => {
  outHeard = false;
};
const pointerHere = () => {
  outHeard = false;
  told = false;
};
const focusGone = () => tell("focus");

/** Heard before anything else hears the move, so a watcher's own move handler finds it armed. */
const BEFORE = { capture: true, passive: true } as const;

function listen() {
  document.documentElement.addEventListener("pointerleave", pointerGone);
  document.addEventListener("pointerout", outIntoNothing);
  document.addEventListener("mouseout", outIntoNothing);
  document.addEventListener("pointerover", pointerOver, BEFORE);
  document.addEventListener("pointermove", pointerHere, BEFORE);
  document.addEventListener("pointerdown", pointerHere, BEFORE);
  window.addEventListener("blur", focusGone);
}

function unlisten() {
  document.documentElement.removeEventListener("pointerleave", pointerGone);
  document.removeEventListener("pointerout", outIntoNothing);
  document.removeEventListener("mouseout", outIntoNothing);
  document.removeEventListener("pointerover", pointerOver, BEFORE);
  document.removeEventListener("pointermove", pointerHere, BEFORE);
  document.removeEventListener("pointerdown", pointerHere, BEFORE);
  window.removeEventListener("blur", focusGone);
  told = false;
  outHeard = false;
}

/** **Tells `watcher` each time** until the function it answers is called, for what is no hook. */
export function watchWindowLeft(watcher: Watcher): () => void {
  // Its own entry, so one function watching twice is two watchers.
  const entry: Watcher = (why) => watcher(why);
  if (watchers.size === 0) listen();
  watchers.add(entry);
  return () => {
    watchers.delete(entry);
    if (watchers.size === 0) unlisten();
  };
}

/** **Tells `onLeft` each time** while `on`, always the one of the last render. */
export function useWindowLeft(onLeft: Watcher, on = true): void {
  const latest = useRef(onLeft);
  useLayoutEffect(() => {
    latest.current = onLeft;
  });
  useEffect(() => {
    if (!on) return;
    return watchWindowLeft((why) => latest.current(why));
  }, [on]);
}
