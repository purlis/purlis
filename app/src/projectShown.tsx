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
 * The projects that may stay drawn while another is in front: the first {@link KEPT_DRAWN} of
 * `recent`, the order the person was last in them, **among the ones the window still holds**. A
 * project that was closed keeps its place in `recent` and takes none here, or three closed
 * projects would leave room for one.
 *
 * May stay, and no more: a view is only ever drawn by its project coming to the front
 * (`PlaneView`), so one named here that draws nothing, such as a project opened again behind
 * the front one, goes on drawing nothing.
 */
export function keptOf(
  recent: readonly string[],
  open: readonly string[],
  most = KEPT_DRAWN,
): ReadonlySet<string> {
  return new Set(recent.filter((plane) => open.includes(plane)).slice(0, most));
}

/**
 * What is drawn over the whole window, wherever it was opened from: a dialog, an alert dialog,
 * a menu and a select's list, each by the role Radix gives it. A tooltip is not one: it asks
 * nothing. `projectShown.guard.test.ts` holds this to every kind of overlay the window uses.
 */
const OVER_THE_WINDOW = [
  ...["dialog", "alertdialog", "menu", "listbox"].map((role) => `[role="${role}"]`),
  // The engine's own dialog element, should one ever be written.
  "dialog[open]",
].join(", ");

/**
 * Whether something is drawn over the window that `slot` is in: outside the window's own
 * element, where a portal puts it, and not the window's palette, which is how a project is
 * switched to and closes with the switch.
 *
 * **A project that leaves the front with something over the window is not kept** (`PlaneView`):
 * the backstop behind the families' surfaces (`useProjectShown`), which keep a project's overlays off the
 * window while it is behind. A dialog a part of its view opened is drawn outside the project's
 * element, so it would stay up over the project that came forward: "Delete persona steward?"
 * asked in one project, answered over another, and done in the first. Should one ever be drawn
 * that does not ask whether its project is shown, whose it is cannot be read off the document,
 * so anything there counts, and the project's view is taken down as every project's used to be:
 * what opened the dialog goes with it.
 */
export function drawnOverTheWindow(slot: Element): boolean {
  let own: Element = slot;
  while (own.parentElement !== null && own.parentElement !== document.body) own = own.parentElement;
  return [...document.querySelectorAll(OVER_THE_WINDOW)].some(
    (over) => !own.contains(over) && over.closest(".palette") === null,
  );
}

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

/** How many scrolled elements a view's places hold before the ones that have gone are let go. */
export const SCROLLED_MOST = 64;

/** How many places are held for the view drawn in `host`: for the test of what is let go. */
export function scrolledPlacesHeld(host: HTMLElement): number {
  return scrolledUnder.get(host)?.size ?? 0;
}

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
  onLeftCovered,
}: {
  host: HTMLElement;
  slot: HTMLElement | null;
  shown: boolean;
  /** Called as the project leaves the front with something drawn over the window
   *  ({@link drawnOverTheWindow}): its view is not to be kept. */
  onLeftCovered: () => void;
}): null {
  const was = useRef(shown);
  useLayoutEffect(() => {
    const before = was.current;
    was.current = shown;
    if (!before || shown || slot === null) return;
    // **Looked at once the press that made the switch is over**, and not in the middle of it: a
    // menu's row switches projects and then closes its menu, in that order and in one press, so
    // the menu is still over the window as the project leaves and gone a moment later. Looked
    // at then, a project left from a menu would lose its view every time.
    let gone = false;
    queueMicrotask(() => {
      if (!gone && drawnOverTheWindow(slot)) onLeftCovered();
    });
    return () => {
      gone = true;
    };
  }, [shown, slot, onLeftCovered]);
  useEffect(() => {
    const places = scrolledIn(host);
    // A scroll does not bubble; it is heard on its way down.
    const moved = (event: Event) => {
      const element = event.target;
      if (!(element instanceof Element)) return;
      const { scrollTop: top, scrollLeft: left } = element;
      if (top === 0 && left === 0) places.delete(element);
      else places.set(element, { top, left });
      // A view in front for hours scrolls many things that have since gone: past so many, the
      // ones no longer in it are let go here, and not only as it leaves the window.
      if (places.size > SCROLLED_MOST)
        for (const one of places.keys()) if (!host.contains(one)) places.delete(one);
    };
    host.addEventListener("scroll", moved, { capture: true, passive: true });
    return () => host.removeEventListener("scroll", moved, { capture: true });
  }, [host]);
  useLayoutEffect(() => {
    if (!shown || slot === null) return;
    slot.appendChild(host);
    scrollBack(host);
    return () => {
      host.remove();
      // What went while the view was in the window is forgotten as it leaves, so nothing that
      // is no longer drawn is held on to for as long as the project is kept.
      const places = scrolledIn(host);
      for (const element of places.keys()) if (!host.contains(element)) places.delete(element);
    };
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
