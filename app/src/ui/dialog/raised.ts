// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **When an alert the app raises may open, and whose turn it is** (#7728, ADR 0104 as amended
 * 2026-11-26): the window's one queue of raised alerts, and what it hears of the person's keys.
 *
 * - **One at a time, one per window.** Alerts are shown in the order they were raised; the rest
 *   wait. Each knows where it stands among those shown since the queue was last empty and those
 *   still waiting ("1 of 2").
 * - **It opens only when the window is quiet**: no IME composition in progress, no key down, and
 *   no dialog (the palette among them) or menu (a select's list among them) up
 *   (`ui/layers.ts`). It waits, and opens when that ends.
 *
 * A key counts as down from its keydown to its keyup, or until {@link KEY_HELD_MS} pass with no
 * keydown of it (a key held repeats; a keyup macOS drops while ⌘ is down never comes). The window
 * losing focus forgets every key and composition: their ends go to another window.
 */
import { useEffect, useId, useSyncExternalStore } from "react";
import { hearLayers, layersUp } from "../layers";

/** How long a key with no keyup still counts as down after its last keydown, in ms. */
export const KEY_HELD_MS = 2000;

/** Each key down, by its code, and when it was last heard. */
const down = new Map<string, number>();
let composing = false;
let forgetting: ReturnType<typeof setTimeout> | undefined;

const keyName = (event: KeyboardEvent) => event.code || event.key;

function aKeyIsDown(): boolean {
  const now = performance.now();
  for (const [key, at] of down) if (now - at >= KEY_HELD_MS) down.delete(key);
  return down.size > 0;
}

const quiet = () =>
  !composing && !aKeyIsDown() && layersUp("dialog") === 0 && layersUp("menu") === 0;

/** The alerts raised, in order; the first is shown once the window is quiet. */
const queue: string[] = [];
let shown: string | null = null;
/** How many were shown and left since the queue was last empty. */
let answered = 0;
/** Alerts let go in this commit, kept until a microtask says they did not come back. */
const letGo = new Set<string>();
const hearing = new Set<() => void>();

const tell = () => {
  for (const heard of [...hearing]) heard();
};

function promote() {
  if (shown !== null || queue.length === 0 || !quiet()) return;
  shown = queue[0];
  tell();
}

function leave(id: string) {
  const at = queue.indexOf(id);
  if (at < 0) return;
  queue.splice(at, 1);
  if (shown === id) {
    shown = null;
    answered += 1;
  }
  if (queue.length === 0) answered = 0;
  tell();
  promote();
}

/**
 * Raises alert `id`: it joins the queue, and leaves it by what this returns. A leave waits a
 * microtask, so React's strict mode taking an effect down and putting it back is no leave.
 */
function raise(id: string): () => void {
  letGo.delete(id);
  if (!queue.includes(id)) {
    queue.push(id);
    tell();
  }
  promote();
  return () => {
    letGo.add(id);
    queueMicrotask(() => {
      if (letGo.delete(id)) leave(id);
    });
  };
}

/** Where alert `id` stands while it is shown, as "<n>/<of>"; nothing while it waits. */
const turnOf = (id: string) => (shown === id ? `${answered + 1}/${answered + queue.length}` : "");

function hear(heard: () => void) {
  hearing.add(heard);
  return () => void hearing.delete(heard);
}

/** **Where a raised alert stands**: shown as `at` of `of`, or `null` while it waits. */
export type Turn = { at: number; of: number } | null;

/** **Raises an alert while `raised`**, and says when its turn has come. */
export function useRaisedTurn(raised: boolean): Turn {
  const id = useId();
  useEffect(() => (raised ? raise(id) : undefined), [raised, id]);
  const turn = useSyncExternalStore(hear, () => turnOf(id));
  if (!raised || turn === "") return null;
  const [at, of] = turn.split("/").map(Number);
  return { at, of };
}

if (typeof window !== "undefined") {
  window.addEventListener(
    "keydown",
    (event) => {
      down.set(keyName(event), performance.now());
      clearTimeout(forgetting);
      forgetting = setTimeout(promote, KEY_HELD_MS);
    },
    true,
  );
  window.addEventListener(
    "keyup",
    (event) => {
      down.delete(keyName(event));
      // macOS sends no keyup for a key let go while ⌘ was down.
      if (event.key === "Meta") down.clear();
      promote();
    },
    true,
  );
  window.addEventListener("compositionstart", () => void (composing = true), true);
  window.addEventListener(
    "compositionend",
    () => {
      composing = false;
      promote();
    },
    true,
  );
  window.addEventListener("blur", () => {
    down.clear();
    composing = false;
    promote();
  });
  hearLayers(promote);
}
