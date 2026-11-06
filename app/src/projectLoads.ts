// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ProjectOutcome } from "./bindings";

/**
 * **Projects load independently at a launch** (#5323, the spec #5319 and its round-2 rulings).
 *
 * The launch's restore used to open each remembered project after the one before it had
 * answered, and a project had no row on the rail until its own open answered, so one slow
 * project held up every one after it and the rail said nothing of what was coming. Now:
 *
 * - every remembered project has its row at once, in its remembered order ({@link railOrder}),
 *   which it keeps whichever project answers first;
 * - the project in front is asked first, at most {@link AT_ONCE} are opening at a time, and the
 *   rest wait their turn ({@link inTurn}); one the person chooses while it waits goes next;
 * - each row says how its project's load is going ({@link loadSaid}) until the core says it is
 *   back (`project-ready`, #5322).
 */

/** How many projects open at once: the one in front and up to three beside it (round 2). */
export const AT_ONCE = 4;

/** How long a project is loading before its row says it is still loading (round 2), in ms. */
export const STILL_LOADING_AFTER = 15_000;

/**
 * How a project's load is going, as its row draws it.
 *
 * `failed` is the core's word for a project none of whose chats were put back, or the window's
 * for one whose open was refused: its row says the error and offers Retry (#5324).
 */
export type ProjectLoad =
  | { kind: "loading"; since: number }
  | { kind: "ready"; back: number; wanted: number }
  | { kind: "failed"; error: string };

/** The core's word on a project's opening (`project-ready`), as a load. */
export function loadOf(outcome: ProjectOutcome): ProjectLoad {
  return outcome.kind === "ready"
    ? { kind: "ready", back: outcome.back, wanted: outcome.wanted }
    : { kind: "failed", error: outcome.error };
}

/** Whether a row draws its loading mark. */
export const isLoading = (load: ProjectLoad | undefined): boolean => load?.kind === "loading";

/** Whether a row draws its failed mark. */
export const isFailed = (load: ProjectLoad | undefined): boolean => load?.kind === "failed";

/**
 * **Whether a row offers Retry** (#5324, round 2): one that failed, one still loading after
 * {@link STILL_LOADING_AFTER}, and one short of chats. Never one loading afresh, so a press has
 * its time to answer, and never one all back.
 */
export function offersRetry(load: ProjectLoad | undefined, now: number): boolean {
  if (load === undefined) return false;
  switch (load.kind) {
    case "loading":
      return now - load.since >= STILL_LOADING_AFTER;
    case "ready":
      return load.back < load.wanted;
    case "failed":
      return true;
  }
}

/** `loads` without the one of `path`: its row says nothing more of it. */
export function withoutLoad(
  loads: Readonly<Record<string, ProjectLoad>>,
  path: string,
): Record<string, ProjectLoad> {
  return Object.fromEntries(Object.entries(loads).filter(([one]) => one !== path));
}

/**
 * What a project's row says of its load, beside its name: nothing once it is all back, and the
 * error, verbatim, once it has failed (#5324).
 *
 * "Still loading…" after {@link STILL_LOADING_AFTER} with no answer: slow is not failed, and is
 * said apart from it (round 2). "N of M chats back" when only some of its chats came back: the
 * project is ready, and not failed (round 2).
 */
export function loadSaid(load: ProjectLoad | undefined, now: number): string | undefined {
  if (load === undefined) return undefined;
  switch (load.kind) {
    case "loading":
      return now - load.since >= STILL_LOADING_AFTER ? "Still loading…" : "Loading…";
    case "ready":
      return load.back < load.wanted ? `${load.back} of ${load.wanted} chats back` : undefined;
    case "failed":
      return load.error;
  }
}

/**
 * **Opens `paths` in turn**: `front` first, then the rest in their order, never more than
 * {@link AT_ONCE} at a time, each next one started as one answers.
 *
 * `toFront` moves a path that is still waiting to the head of the queue, so it is the next one
 * opened; a path already opening is left as it is. `done` answers once every open has, with
 * each one's answer in `paths`' order, and `undefined` for one that threw.
 */
export function inTurn<T>(
  paths: readonly string[],
  front: string | undefined,
  open: (path: string) => Promise<T>,
  atOnce = AT_ONCE,
): {
  done: Promise<(T | undefined)[]>;
  toFront: (path: string) => void;
  /** Whether `path` is still waiting its turn. */
  waits: (path: string) => boolean;
} {
  const waiting =
    front !== undefined && paths.includes(front)
      ? [front, ...paths.filter((path) => path !== front)]
      : [...paths];
  const answers = new Map<string, Promise<T | undefined>>();
  let finished!: () => void;
  const allStarted = new Promise<void>((resolve) => (finished = resolve));
  let opening = 0;
  const next = () => {
    while (opening < atOnce && waiting.length > 0) {
      const path = waiting.shift() as string;
      opening += 1;
      answers.set(
        path,
        open(path)
          .catch(() => undefined)
          .finally(() => {
            opening -= 1;
            next();
          }),
      );
    }
    if (waiting.length === 0) finished();
  };
  next();
  return {
    done: allStarted.then(() => Promise.all(paths.map((path) => answers.get(path)))),
    toFront: (path) => {
      const at = waiting.indexOf(path);
      if (at <= 0) return;
      waiting.splice(at, 1);
      waiting.unshift(path);
    },
    waits: (path) => waiting.includes(path),
  };
}

/**
 * **The rail's order while projects load**: the remembered order, each project in it that has
 * opened or is still coming, then any other project the window holds in the order it holds
 * them. A remembered project that answered with nothing (it asked first, or is gone) has no row.
 */
export function railOrder(
  remembered: readonly string[],
  held: readonly string[],
  coming: readonly string[],
): string[] {
  const rows = remembered.filter((one) => held.includes(one) || coming.includes(one));
  return [...rows, ...held.filter((one) => !rows.includes(one))];
}

/**
 * **`plane` put among `held` at its remembered place**, as its open answers: before the first
 * project held that was remembered after it, or last. One already held stays where it is, and
 * one not remembered (opened by hand) goes last.
 */
export function placed(
  held: readonly string[],
  plane: string,
  remembered: readonly string[],
): string[] {
  if (held.includes(plane)) return [...held];
  const own = remembered.indexOf(plane);
  const before = own < 0 ? -1 : held.findIndex((one) => remembered.indexOf(one) > own);
  return before < 0 ? [...held, plane] : [...held.slice(0, before), plane, ...held.slice(before)];
}
