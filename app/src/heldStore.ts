// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a project's chats are doing, held outside React and told to its readers once a
 * frame** (#4731): the one store `chatState.ts`, `chatDoing.ts` and `skillRuns.ts` each keep.
 *
 * With many busy agents, `chat-moved`, `chat-doing` and `chat-skill-run` arrive dozens of times
 * a second, each as a task of its own. Told to the readers as each arrived, every one was a
 * commit, and a window behind on its commits is a slow window. Here a change is applied at once,
 * in the order it came, to what is held next; the readers are told once, at the next frame, and
 * see everything that arrived before it in one commit. Nothing is dropped or reordered: it is
 * only told later.
 *
 * **What a reader is given changes only when it is told** (`useSyncExternalStore`'s rule: the
 * same snapshot until the store says otherwise), so what is applied between two frames is held
 * aside until the frame.
 *
 * **A window that draws no frames still catches up.** A hidden or minimised webview runs no
 * animation frames, and something may read the store there (the dock's count of chats waiting
 * on the person), so a timer tells the readers if no frame has within {@link AT_MOST_MS}.
 */
export type HeldStore<T> = {
  subscribe: (listener: () => void) => () => void;
  /** What the readers were last told. */
  held: () => T;
  /** Applies `how` to what is held, and tells the readers at the next frame if it changed. */
  change: (how: (was: T) => T) => void;
};

/** The longest the readers wait to be told when no frame is drawn. */
export const AT_MOST_MS = 250;

/** The stores with something to tell, told together at the next frame. */
const waiting = new Set<() => void>();
let frame: number | undefined;
let timer: ReturnType<typeof setTimeout> | undefined;

/**
 * **Every store is told in the one frame callback**, so the moves, the doings and the runs that
 * arrived together are one commit and not one a store: React commits what one task asked of it,
 * and each frame callback is a task of its own. Exported for a test that draws the frame at once
 * (`frame.testkit.ts`).
 */
export function tellAll() {
  if (frame !== undefined) cancelAnimationFrame(frame);
  clearTimeout(timer);
  frame = timer = undefined;
  const now = [...waiting];
  waiting.clear();
  for (const tell of now) tell();
}

function atTheNextFrame(tell: () => void) {
  waiting.add(tell);
  if (timer !== undefined) return;
  frame = requestAnimationFrame(tellAll);
  timer = setTimeout(tellAll, AT_MOST_MS);
}

export function heldStore<T>(first: T): HeldStore<T> {
  let told = first;
  /** What is held after every change since the readers were last told: `told` when none. */
  let next = first;
  const listeners = new Set<() => void>();

  const tell = () => {
    if (next === told) return;
    told = next;
    for (const listener of [...listeners]) listener();
  };

  return {
    subscribe: (listener) => {
      listeners.add(listener);
      return () => void listeners.delete(listener);
    },
    held: () => told,
    change: (how) => {
      next = how(next);
      if (next !== told) atTheNextFrame(tell);
    },
  };
}
