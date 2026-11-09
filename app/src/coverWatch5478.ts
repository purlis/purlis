// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * [DEBUG-5478] **Temporary diagnostics for #5478: what takes a frozen pane's keys and mouse.**
 *
 * During a real freeze (2026-11-09, dev.2070) typing, the mouse and text selection all did
 * nothing in one pane, and the freeze watchdog (`freezeWatch5478.ts`) wrote no line. That
 * watchdog starts only on what xterm passes on, so the events most likely never reached that
 * pane's xterm at all. This finds out what they reached instead.
 *
 * One window-level listener, on the capture phase, for `pointerdown` and `keydown`:
 *
 * - **A press inside a shown pane's terminal whose target is not that pane's live `.xterm`**
 *   (the element its Terminal owns, `pane.element`) writes one line: the top
 *   {@link STACK_KEPT} elements at that point (`document.elementsFromPoint`) with their tag,
 *   class, `data-*` names and computed `pointer-events`, `z-index` and `opacity`;
 *   `document.activeElement`; and whether the pane's `.xterm` is in the page and is the one
 *   its Terminal owns, rather than a kept or stale copy (`keptTerminals.ts`).
 * - **A key pressed while the keyboard is in no terminal and no field, the last press having
 *   been on a terminal**, writes the same, at the centre of that pane.
 * - **The first event to reach a pane's xterm again**, after one of those, writes how long it
 *   was missed.
 *
 * At most one line per pane per {@link AT_MOST_EVERY_MS}; the recovery line is the follow-up
 * to one of those and comes at most once for each. It changes nothing: it only listens and
 * reads. Everything tagged `[DEBUG-5478]` goes with #5488.
 */
import type { Terminal } from "@xterm/xterm";

/** [DEBUG-5478] The least time between two lines of one pane, the recovery line aside. */
export const AT_MOST_EVERY_MS = 60_000;

/** [DEBUG-5478] How many of the elements under the point a line names, the topmost first. */
export const STACK_KEPT = 6;

/** [DEBUG-5478] A pane the listener watches: its holder, and the Terminal it shows now. */
interface Watched {
  session: number;
  holder: HTMLElement;
  terminal: () => Pick<Terminal, "element"> | undefined;
  log: (line: string) => void;
  lastLine?: number;
  /** When an event first missed this pane's xterm, until one reaches it again. */
  missedSince?: number;
}

const watched = new Set<Watched>();
/** The pane the last press landed in, reached or not: whose keys a stray key would be. */
let lastPressed: Watched | undefined;
let now: () => number = () => Date.now();

/** [DEBUG-5478] A test's clock in place of the browser's. Gives back the browser's. */
export function clockForTest(clock: () => number): () => void {
  now = clock;
  return () => {
    now = () => Date.now();
  };
}

const isInside = (box: DOMRect, x: number, y: number) =>
  box.width > 0 &&
  box.height > 0 &&
  x >= box.left &&
  x < box.right &&
  y >= box.top &&
  y < box.bottom;

/** One element as a line names it. */
function named(element: Element | null): Record<string, unknown> | null {
  if (element === null) return null;
  const data = Object.keys((element as HTMLElement).dataset ?? {});
  let style: CSSStyleDeclaration | undefined;
  try {
    style = getComputedStyle(element);
  } catch {
    // Read for the line alone.
  }
  return {
    tag: element.tagName.toLowerCase(),
    class: typeof element.className === "string" ? element.className.slice(0, 120) : null,
    data,
    pointerEvents: style?.pointerEvents ?? null,
    zIndex: style?.zIndex ?? null,
    opacity: style?.opacity ?? null,
  };
}

/** What is under `x`, `y`, the topmost first. */
function stackAt(x: number, y: number): unknown[] {
  if (typeof document.elementsFromPoint !== "function") return [];
  try {
    return document.elementsFromPoint(x, y).slice(0, STACK_KEPT).map(named);
  } catch {
    return [];
  }
}

/** The pane's terminal as the page has it: which `.xterm` is shown, and whose. */
function paneState(pane: Watched) {
  const owned = pane.terminal()?.element;
  const shown = [...pane.holder.querySelectorAll(".xterm")];
  return {
    session: pane.session,
    holderConnected: pane.holder.isConnected,
    hasTerminal: owned !== undefined,
    ownedConnected: owned?.isConnected ?? null,
    ownedInHolder: owned !== undefined && pane.holder.contains(owned),
    xtermsInHolder: shown.length,
    shownIsOwned: shown.length > 0 && shown.every((el) => el === owned),
  };
}

function write(pane: Watched, said: string) {
  try {
    pane.log(said);
  } catch {
    // A line that cannot be written is let go: it is a record, never a reason to stop.
  }
}

/** An event missed `pane`: one line, if its minute allows. */
function missed(pane: Watched, what: string, target: EventTarget | null, x: number, y: number) {
  const at = now();
  if (pane.lastLine !== undefined && at - pane.lastLine < AT_MOST_EVERY_MS) return;
  pane.lastLine = at;
  // Armed only by a line written, so a recovery line always follows one.
  pane.missedSince ??= at;
  write(
    pane,
    `pane missed ${what}: ${JSON.stringify({
      ...paneState(pane),
      at: { x: Math.round(x), y: Math.round(y) },
      target: target instanceof Element ? named(target) : null,
      activeElement: named(document.activeElement),
      stack: stackAt(x, y),
    })}`,
  );
}

/** An event reached `pane`'s xterm: after a miss, how long it took. */
function reached(pane: Watched, what: string) {
  if (pane.missedSince === undefined) return;
  const ms = now() - pane.missedSince;
  pane.missedSince = undefined;
  write(
    pane,
    `pane recovered after ${Math.round(ms / 100) / 10} s: ${JSON.stringify({ session: pane.session, by: what, missedMs: ms })}`,
  );
}

function onPointerDown(event: PointerEvent) {
  const target = event.target instanceof Node ? event.target : null;
  let landed: Watched | undefined;
  for (const pane of watched) {
    if (!pane.holder.isConnected) continue;
    if (!isInside(pane.holder.getBoundingClientRect(), event.clientX, event.clientY)) continue;
    landed = pane;
    const owned = pane.terminal()?.element;
    if (owned !== undefined && target !== null && owned.contains(target)) {
      reached(pane, "pointerdown");
      continue;
    }
    // The holder's own padding is no terminal, unless the terminal is not in it at all.
    if (target === pane.holder && owned !== undefined && pane.holder.contains(owned)) continue;
    missed(pane, "pointerdown", event.target, event.clientX, event.clientY);
  }
  lastPressed = landed;
}

/** Where a key goes on purpose that is not a terminal: a field, or something editable. */
function isAField(element: Element | null): boolean {
  if (!(element instanceof HTMLElement)) return false;
  if (element.closest(".xterm")) return false;
  return (
    element instanceof HTMLInputElement ||
    element instanceof HTMLTextAreaElement ||
    element instanceof HTMLSelectElement ||
    element.isContentEditable
  );
}

function onKeyDown(event: KeyboardEvent) {
  const active = document.activeElement;
  for (const pane of watched) {
    const owned = pane.terminal()?.element;
    if (owned !== undefined && active !== null && owned.contains(active)) {
      reached(pane, "keydown");
      return;
    }
  }
  const pane = lastPressed;
  if (pane === undefined || !watched.has(pane) || !pane.holder.isConnected) return;
  if (isAField(active)) return;
  const box = pane.holder.getBoundingClientRect();
  missed(pane, "keydown", event.target, box.left + box.width / 2, box.top + box.height / 2);
}

/**
 * [DEBUG-5478] Watches the pane drawn in `holder` for events that land on it and miss its
 * terminal, `terminal()` giving the one it shows now. Gives back its leaving.
 */
export function watchForCover(
  holder: HTMLElement,
  session: number,
  terminal: () => Pick<Terminal, "element"> | undefined,
  log: (line: string) => void,
): () => void {
  const pane: Watched = { session, holder, terminal, log };
  if (watched.size === 0) {
    window.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("keydown", onKeyDown, true);
  }
  watched.add(pane);
  return () => {
    watched.delete(pane);
    if (lastPressed === pane) lastPressed = undefined;
    if (watched.size === 0) {
      window.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("keydown", onKeyDown, true);
    }
  };
}
