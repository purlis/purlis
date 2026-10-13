// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  createContext,
  memo,
  useContext,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";

/**
 * **A project that is not in front stays drawn, off the window** (`PlaneView`).
 *
 * Switching projects used to take the whole view of one down and build the other's again:
 * every strip, list and panel mounted, every read it makes at its mount asked of the core, the
 * terminal of its chat disposed and a new one started and sent the screen. A person felt that
 * as a freeze, longer the more chats the project had.
 *
 * So each project draws into an element of its own (`host`), and only the project in front's is
 * in the window (`slot`). The rest keep everything they drew and every terminal they started,
 * in an element that is in no document: nothing lays it out or paints it, no query of the
 * window finds it, no key or press reaches it, and bringing it back is putting one element in.
 *
 * **What a part of the view does only while a person can see it** reads {@link useProjectShown}:
 * a read on a beat, a read when the window is focused again, taking the keyboard.
 *
 * **What it shows as it comes back is what it held, and it is read again behind that**
 * ({@link useOnShownAgain}): building the view again used to read everything again on every
 * switch, and a part that reads on a beat, on the window's focus or on a change on disk still
 * owes that read, since nothing else may ever tell it what changed while it was away. It is
 * made once the project has been painted, so no switch waits on a read.
 */
const ProjectShown = createContext(true);

/** Says whether the project this is drawn in is the one in front. */
export const ProjectShownProvider = ProjectShown.Provider;

/**
 * Whether the project this is drawn in is the one in front; `true` outside any project, where
 * what is drawn is the window's own.
 */
export function useProjectShown(): boolean {
  return useContext(ProjectShown);
}

/** Runs `run` once the frame being drawn now has been painted. Answers how to call it off. */
function afterPaint(run: () => void): () => void {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const frame = requestAnimationFrame(() => {
    timer = setTimeout(run, 0);
  });
  return () => {
    cancelAnimationFrame(frame);
    if (timer !== undefined) clearTimeout(timer);
  };
}

/**
 * Calls `run` each time the project this is drawn in comes back to the front, once it has been
 * painted there: where a part reads again what it held while it was away. Never at its mount,
 * which reads for itself, and never outside a project. The latest `run` is the one called.
 */
export function useOnShownAgain(run: () => void): void {
  const shown = useProjectShown();
  const latest = useRef(run);
  useEffect(() => {
    latest.current = run;
  });
  const was = useRef(shown);
  useEffect(() => {
    const before = was.current;
    was.current = shown;
    if (!shown || before) return;
    return afterPaint(() => latest.current());
  }, [shown]);
}

/** How many projects stay drawn at once: the one in front and the ones the person was in last
 *  before it. One they have not been in for longer is drawn again when they come back to it,
 *  so what a window keeps is bounded by this and not by how many projects it holds. Each one
 *  kept holds its view's elements and the terminals of the tab it had in front. */
export const KEPT_DRAWN = 4;

/**
 * The element a project draws into, made once and kept for as long as its view is.
 *
 * `display: contents` (`.project-view`), so what is drawn in it is laid out as the window's own
 * rows, as it was before there was an element here.
 */
export function useProjectHost(): HTMLElement {
  const [host] = useState(() => {
    const made = document.createElement("div");
    made.className = "project-view";
    return made;
  });
  return host;
}

/** Where each scrolled element under a host is scrolled to. */
type Scrolled = Map<Element, { top: number; left: number }>;

/** What is scrolled under each host, kept with the host and gone with it. */
const scrolledUnder = new WeakMap<HTMLElement, Scrolled>();

function scrolledIn(host: HTMLElement): Scrolled {
  let places = scrolledUnder.get(host);
  if (places === undefined) {
    places = new Map();
    scrolledUnder.set(host, places);
  }
  return places;
}

/** Scrolls everything under `host` back to where it was written down. */
function scrollBack(host: HTMLElement): void {
  const places = scrolledIn(host);
  for (const [element, { top, left }] of places) {
    // Only one still drawn in this view: one that went meanwhile has nowhere to scroll.
    if (!host.contains(element)) {
      places.delete(element);
      continue;
    }
    element.scrollTop = top;
    element.scrollLeft = left;
  }
}

/**
 * Puts a project's `host` in the window's `slot` while it is the one in front, and takes it out
 * when it is not.
 *
 * **A component of its own, drawn first in the host**: React runs a layout effect of an earlier
 * sibling before a later one's, so the host is in the window before anything drawn after this
 * measures itself there.
 *
 * **What was scrolled is scrolled there again.** An element taken out of the document forgets
 * how far it was scrolled, so each list, tree and file would come back at its top. Where each
 * is, is written down as it is scrolled, and put back as the host goes in, before anything is
 * painted. Written down then, and not read as the host is taken out: a read there would lay the
 * whole window out once more, for a view that is leaving it.
 */
export function InTheWindow({
  host,
  slot,
  shown,
}: {
  host: HTMLElement;
  slot: HTMLElement | null;
  shown: boolean;
}): null {
  useEffect(() => {
    const places = scrolledIn(host);
    // A scroll does not bubble; it is heard on its way down.
    const moved = (event: Event) => {
      const element = event.target;
      if (!(element instanceof Element)) return;
      const { scrollTop: top, scrollLeft: left } = element;
      if (top === 0 && left === 0) places.delete(element);
      else places.set(element, { top, left });
    };
    host.addEventListener("scroll", moved, { capture: true, passive: true });
    return () => host.removeEventListener("scroll", moved, { capture: true });
  }, [host]);
  useLayoutEffect(() => {
    if (!shown || slot === null) return;
    slot.appendChild(host);
    scrollBack(host);
    return () => host.remove();
  }, [host, slot, shown]);
  return null;
}

/**
 * **What a project drew when it was last in front, until it is in front again.**
 *
 * A project that is not in front goes on hearing about its chats, and the window draws it again
 * for each thing it hears (`PlaneView`). Nothing of that can be seen, so nothing under here is
 * drawn again for it: this keeps the children it was last given while the project was shown,
 * and takes new ones the moment it is shown again. A terminal is not drawn by React, so a chat
 * over there goes on receiving what its program writes.
 *
 * Whatever reads {@link useProjectShown} is still told the moment the project leaves the front
 * and the moment it is back: a context reaches past this.
 */
export function AsLastShown({ children }: { children: ReactNode }): ReactNode {
  return <LastShown shown={useProjectShown()}>{children}</LastShown>;
}

const LastShown = memo(
  function LastShown({ children }: { shown: boolean; children: ReactNode }): ReactNode {
    return children;
  },
  // "Equal", so not drawn again, whenever the project is not shown.
  (_, next) => !next.shown,
);

/**
 * Draws its children only while the project is in front: what is asked over the whole window,
 * such as a dialog, which is drawn outside the project's own element and would otherwise stay
 * up over another project. What opened it is the project's state, so it is there again when
 * the project is.
 */
export function OnlyInFront({ children }: { children: ReactNode }): ReactNode {
  return useProjectShown() ? children : null;
}
