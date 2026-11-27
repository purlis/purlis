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
 *   (`ui/layers.ts`). It waits, and opens when that ends. A keyup that ends the wait is heard
 *   before the press it ends (a Space let go on a button that opens a dialog), and a layer that
 *   closes is uncounted before a dialog drawn in the same commit counts itself (React runs a
 *   commit's effect cleanups before its new effects). So after either, and after a raise, the
 *   alert opens a task later, once what that commit drew has counted itself, and only if the
 *   window is still quiet.
 * - **An alert whose project goes behind leaves its turn, unanswered**: it is not counted among
 *   those shown, and as its project comes back it takes its place again by when it was raised.
 *
 * A key counts as down from its keydown to its keyup, or until {@link KEY_HELD_MS} pass with no
 * keydown of it (a key held repeats; a keyup macOS drops while ⌘ is down never comes). The window
 * losing focus forgets every key and composition: their ends go to another window. So does the
 * field a composition began in leaving the document: its end never comes.
 */
import { useEffect, useId, useSyncExternalStore } from "react";
import { hearLayers, layersUp } from "../layers";

/** How long a key with no keyup still counts as down after its last keydown, in ms. */
export const KEY_HELD_MS = 2000;

/** How long after a key is forgotten the queue looks again: a timer may run a little early. */
const FORGET_SLACK_MS = 50;

/** Each key down, by its code, and when it was last heard. */
const down = new Map<string, number>();
let composing = false;
/** Watches for the field a composition began in leaving the document, while it composes. */
let composingIn: MutationObserver | undefined;
let forgetting: ReturnType<typeof setTimeout> | undefined;
let promoting: ReturnType<typeof setTimeout> | undefined;

/** Looks again a task later: once what the commit or press in progress draws has counted itself. */
function promoteLater() {
  clearTimeout(promoting);
  promoting = setTimeout(promote, 0);
}

const keyName = (event: KeyboardEvent) => event.code || event.key;

function aKeyIsDown(): boolean {
  const now = performance.now();
  for (const [key, at] of down) if (now - at >= KEY_HELD_MS) down.delete(key);
  return down.size > 0;
}

/** Looks again once the last key heard is forgotten, and again if the timer ran early. */
function forgetLater() {
  clearTimeout(forgetting);
  if (down.size === 0) return;
  const left = Math.max(...down.values()) + KEY_HELD_MS - performance.now();
  forgetting = setTimeout(
    () => (aKeyIsDown() ? forgetLater() : promote()),
    Math.max(0, left) + FORGET_SLACK_MS,
  );
}

function composed() {
  composing = false;
  composingIn?.disconnect();
  composingIn = undefined;
}

function composingStarted(event: Event) {
  composing = true;
  composingIn?.disconnect();
  const field = event.target;
  if (!(field instanceof Node) || typeof MutationObserver === "undefined") return;
  composingIn = new MutationObserver(() => {
    if (field.isConnected) return;
    composed();
    promote();
  });
  composingIn.observe(field.ownerDocument ?? document, { childList: true, subtree: true });
}

const quiet = () =>
  !composing && !aKeyIsDown() && layersUp("dialog") === 0 && layersUp("menu") === 0;

/** The alerts raised and drawable, in the order raised; the first is shown once the window is quiet. */
const queue: string[] = [];
let shown: string | null = null;
/** When each alert still raised was raised, whether its project is in front or not. */
const raisedAt = new Map<string, number>();
let raisedSoFar = 0;
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
    // One still raised went behind with its project: it was not answered.
    if (!raisedAt.has(id)) answered += 1;
  }
  if (queue.length === 0) answered = 0;
  tell();
  promote();
}

/** Holds alert `id`'s place in the order raised while it is raised; gives it up by what this returns. */
function hold(id: string): () => void {
  if (!raisedAt.has(id)) raisedAt.set(id, (raisedSoFar += 1));
  return () => void raisedAt.delete(id);
}

/** Puts `id` in the queue by when it was raised, behind the one shown, which stays first. */
function join(id: string) {
  const when = raisedAt.get(id) ?? Infinity;
  let at = shown === null ? 0 : 1;
  while (at < queue.length && (raisedAt.get(queue[at]) ?? Infinity) < when) at += 1;
  queue.splice(at, 0, id);
}

/**
 * Draws alert `id` in its turn: it joins the queue, and leaves it by what this returns. A leave
 * waits a microtask, so React's strict mode taking an effect down and putting it back is no leave.
 */
function raise(id: string): () => void {
  letGo.delete(id);
  if (!queue.includes(id)) {
    join(id);
    tell();
  }
  // A task later: every alert this commit raises or brings back has joined first.
  promoteLater();
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

/**
 * **Raises an alert while `raised`**, and says when its turn has come. It takes a turn only while
 * it can be drawn (`inFront`, its project in front); one behind keeps when it was raised, and
 * leaving as its project goes behind is no answer.
 */
export function useRaisedTurn(raised: boolean, inFront: boolean): Turn {
  const id = useId();
  // Before the turn's own effect: an alert no longer raised has given up its place as it leaves.
  useEffect(() => (raised ? hold(id) : undefined), [raised, id]);
  const drawable = raised && inFront;
  useEffect(() => (drawable ? raise(id) : undefined), [drawable, id]);
  const turn = useSyncExternalStore(hear, () => turnOf(id));
  if (!drawable || turn === "") return null;
  const [at, of] = turn.split("/").map(Number);
  return { at, of };
}

if (typeof window !== "undefined") {
  window.addEventListener(
    "keydown",
    (event) => {
      down.set(keyName(event), performance.now());
      forgetLater();
    },
    true,
  );
  window.addEventListener(
    "keyup",
    (event) => {
      down.delete(keyName(event));
      // macOS sends no keyup for a key let go while ⌘ was down.
      if (event.key === "Meta") down.clear();
      // A task later: this keyup's own press (React Aria's, on the same keyup) comes first.
      promoteLater();
    },
    true,
  );
  window.addEventListener("compositionstart", composingStarted, true);
  window.addEventListener(
    "compositionend",
    () => {
      composed();
      promote();
    },
    true,
  );
  window.addEventListener("blur", () => {
    down.clear();
    composed();
    promote();
  });
  // A task later: a dialog opened in the commit that closed a layer has not counted itself yet.
  hearLayers(promoteLater);
}
