// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A view tab's own history** (#3667, spec #2897): where the person has been inside one view
 * tab, so back and forward (⌘[ and ⌘], the mouse's own buttons, the bar's two buttons) walk it.
 *
 * - **It belongs to the tab.** It is kept by the tab's id ({@link useViewHistory}), so another
 *   tab never shares it, and a view drawn again (its tab back in front) finds it where it was.
 *   It is kept in memory only: a relaunch starts every tab's history afresh.
 * - **A page is the owner's**: whatever the view needs to draw a place again (a page, the item
 *   chosen on it, the page tab chosen in that item), compared by `same`.
 * - **Going somewhere from outside the tab** (the run chip's Runs, a chat notice) is
 *   {@link openViewAt}: on a tab with a history the place is gone to, so back returns to where
 *   the person was inside the tab; on a tab with none, it is where the history starts, and there
 *   is no back.
 */
import { useCallback, useSyncExternalStore } from "react";

/** **Where a tab has been**: the places before this one, this one, and those back went past. */
export interface History<P> {
  readonly back: readonly P[];
  readonly at: P;
  readonly forward: readonly P[];
}

/** How two places are told apart: by value, when the owner says nothing. */
export type Same<P> = (a: P, b: P) => boolean;

/** **Two places are one** when they hold the same values, whatever order their keys were
 *  written in. */
function byValue<P>(a: P, b: P): boolean {
  if (Object.is(a, b)) return true;
  if (typeof a !== "object" || typeof b !== "object" || a === null || b === null) return false;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  const one = Object.entries(a).filter(([, value]) => value !== undefined);
  const other = Object.entries(b).filter(([, value]) => value !== undefined);
  if (one.length !== other.length) return false;
  const theirs = new Map(other);
  return one.every(([key, value]) => theirs.has(key) && byValue(value, theirs.get(key)));
}

/** **A history that starts at `at`**: nothing to go back or forward to. */
export function started<P>(at: P): History<P> {
  return { back: [], at, forward: [] };
}

/** **Gone to `to`**: where the person was is behind them, and what was ahead is forgotten. Going
 *  to where they already are changes nothing. */
export function wentTo<P>(history: History<P>, to: P, same: Same<P> = byValue): History<P> {
  if (same(history.at, to)) return history;
  return { back: [...history.back, history.at], at: to, forward: [] };
}

/** **Where they are, said again** (a page tab chosen inside an item): no new step. */
export function replaced<P>(history: History<P>, to: P): History<P> {
  return { ...history, at: to };
}

/** **One step back**, or the history as it was at its start. */
export function wentBack<P>(history: History<P>): History<P> {
  const to = history.back.at(-1);
  if (to === undefined) return history;
  return { back: history.back.slice(0, -1), at: to, forward: [history.at, ...history.forward] };
}

/** **One step forward**, or the history as it was at its end. */
export function wentForward<P>(history: History<P>): History<P> {
  const [to, ...rest] = history.forward;
  if (to === undefined) return history;
  return { back: [...history.back, history.at], at: to, forward: rest };
}

// --- Kept by the tab ---------------------------------------------------------------------------

/** Every tab's history, by the tab's id, for as long as the window runs. */
const kept = new Map<string, History<unknown>>();
const listeners = new Set<() => void>();

function keep(tab: string, history: History<unknown>): void {
  if (kept.get(tab) === history) return;
  kept.set(tab, history);
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/**
 * **A place reached from outside the tab** (#3667): on a tab with a history, gone to, so back
 * returns to where the person was inside it; on a tab with none, where its history starts.
 */
export function openViewAt<P>(tab: string, at: P, same: Same<P> = byValue): void {
  const held = kept.get(tab) as History<P> | undefined;
  keep(tab, held === undefined ? started(at) : wentTo(held, at, same));
}

/** **The tab has closed**: its history goes with it. Whoever closes a view tab that keeps a
 *  history calls this with its id (the Skill studio's tab, #3681). */
export function forgetViewHistory(tab: string): void {
  if (!kept.delete(tab)) return;
  for (const listener of listeners) listener();
}

/** **A view tab's history, as its view walks it.** */
export interface ViewHistory<P> {
  /** Where the person is now. */
  at: P;
  canGoBack: boolean;
  canGoForward: boolean;
  /** Go to `to`: a new step, which forgets what was ahead. */
  go: (to: P) => void;
  /** Say again where they are, with no new step: a page tab chosen inside an item. */
  replace: (to: P) => void;
  back: () => void;
  forward: () => void;
}

/** **What back and forward need of a history**, whatever its places are: the bar's buttons, the
 *  frame's keys and the mouse's buttons. */
export type HistorySteps = Pick<
  ViewHistory<unknown>,
  "canGoBack" | "canGoForward" | "back" | "forward"
>;

/**
 * **The history of the tab `tab`**, which starts at `start` the first time the tab draws it and
 * is found again every time after, until {@link forgetViewHistory}.
 */
export function useViewHistory<P>(tab: string, start: P, same: Same<P> = byValue): ViewHistory<P> {
  const history = useSyncExternalStore(
    subscribe,
    () => (kept.get(tab) as History<P> | undefined) ?? null,
  );
  // `start` is read only while the tab has no history: the history is the tab's from then on.
  const now = history ?? started(start);
  const write = useCallback(
    (step: (was: History<P>) => History<P>) => {
      const was = (kept.get(tab) as History<P> | undefined) ?? started(start);
      keep(tab, step(was) as History<unknown>);
    },
    [tab, start],
  );
  return {
    at: now.at,
    canGoBack: now.back.length > 0,
    canGoForward: now.forward.length > 0,
    go: useCallback((to: P) => write((was) => wentTo(was, to, same)), [write, same]),
    replace: useCallback((to: P) => write((was) => replaced(was, to)), [write]),
    back: useCallback(() => write(wentBack), [write]),
    forward: useCallback(() => write(wentForward), [write]),
  };
}
