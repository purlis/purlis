// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Quiet** (#6017, ADR 0121 decision 13, the glossary's term): notifications held, never the
 * count. One idea from three sources: the schedule (quiet hours, Settings › You ›
 * Attention), "Quiet now" by hand (the toggle at the top of Asks, and the palette's row),
 * and the person typing in a chat, until about half a minute after their last key. The core
 * holds it (`asknotify.rs`, `QuietSources`): it sees every key a pane sends, and reads the
 * schedule. While Quiet the ✋ still counts and nothing else in the window changes.
 *
 * **When Quiet ends, one summary heads Asks** ({@link QuietSummaryOf}): "While you were quiet:
 * 3 asks, 2 finished, 1 failed", each part a link, until all of it has been seen (#6002). The
 * core says what came in while it held (`Ended`); the updates of the same span are counted here.
 *
 * Not a "focus mode": that name is #2921's, the window with its rail and bars put away.
 */
import { useSyncExternalStore } from "react";
import {
  commands,
  type InboxUpdate,
  type PlaneId,
  type QuietNow,
  type QuietEnded,
  type Shown,
} from "./bindings";
import { listen } from "./here";
import { askSeenKey, updateSeenKey } from "./seenRows";

/** The event the core sends every window when Quiet starts or ends (`QUIET_CHANGED`). */
export const QUIET_CHANGED = "quiet-changed";

/** What the toggle and the palette's row are called. */
export const QUIET_NOW = "Quiet now";

/** What the toggle's hint says it does. */
export const QUIET_NOW_TIP = "Hold notifications until you turn this off. The ✋ still counts.";

const NOT_QUIET: QuietNow = { quiet: false, by_hand: false, ended: null };

let current: QuietNow = NOT_QUIET;
const listeners = new Set<() => void>();
let following: Promise<unknown> | undefined;

const isQuietNow = (said: unknown): said is QuietNow =>
  typeof said === "object" &&
  said !== null &&
  typeof (said as QuietNow).quiet === "boolean" &&
  typeof (said as QuietNow).by_hand === "boolean";

function told(said: unknown) {
  if (!isQuietNow(said)) return;
  current = said;
  for (const one of listeners) one();
}

/** Follows the core's Quiet, once for the window: asked once, then told of each change. */
function follow() {
  if (following !== undefined) return;
  following = Promise.all([
    listen<unknown>(QUIET_CHANGED, (event) => told(event.payload)),
    commands.quietNow().then(told),
  ]).catch(() => undefined);
}

/** Whether it is Quiet, and what the latest Quiet that ended held, as the core said last. Asked
 *  of the core the first time, so a reader that does not redraw with it still hears it. */
export function quietNow(): QuietNow {
  follow();
  return current;
}

/** {@link quietNow}, for a component that redraws as it changes. */
export function useQuiet(): QuietNow {
  return useSyncExternalStore(
    (changed) => {
      follow();
      listeners.add(changed);
      return () => listeners.delete(changed);
    },
    () => current,
  );
}

/** Turns "Quiet now" on or off. Drawn so at once; the core's word follows. */
export function setQuietNow(on: boolean): void {
  told({ ...current, by_hand: on, quiet: on || current.quiet });
  void commands.setQuietNow(on).catch(() => undefined);
}

/** How often a pane tells the core the person is typing: well inside the core's half a minute
 *  (`TYPING_QUIET`), so Quiet holds while they type and ends that long after they stop. */
export const TYPING_SAID_EVERY_MS = 5_000;

let typingSaidAt = Number.NEGATIVE_INFINITY;
let typingAsking = false;

/**
 * **The person typed `text` into a chat's pane**: the core is told at most every
 * {@link TYPING_SAID_EVERY_MS}. It answers whether the text was them, not the terminal
 * answering by itself, and only text that was them starts the next wait.
 */
export function typedInPane(text: string, now: number = Date.now()): void {
  if (typingAsking || now - typingSaidAt < TYPING_SAID_EVERY_MS) return;
  typingAsking = true;
  void commands
    .quietTyped(text)
    .then((counted) => {
      if (counted === true) typingSaidAt = now;
    })
    .catch(() => undefined)
    .finally(() => {
      typingAsking = false;
    });
}

/** **What a Quiet that ended still has to show in one project**: its parts, each New. */
export type QuietSummaryOf = {
  /** The asks that came in while it held, still waiting and not seen. */
  asks: readonly Shown[];
  /** The tasks that finished while it held, not seen. */
  finished: readonly InboxUpdate[];
  /** The tasks that failed while it held, not seen. */
  failed: readonly InboxUpdate[];
};

/**
 * **The summary of `ended` in project `plane`**, from its asks and updates as they stand now:
 * only what is still New ({@link isNew}, #6002), so it shrinks as the person sees each part and
 * is empty once all of it is seen. An update marked read is seen.
 */
export function quietSummaryOf(
  ended: QuietEnded | null | undefined,
  plane: PlaneId,
  asks: readonly Shown[] | undefined,
  updates: readonly InboxUpdate[] | undefined,
  isNew: (key: string) => boolean,
): QuietSummaryOf {
  if (ended === null || ended === undefined) return { asks: [], finished: [], failed: [] };
  const came = new Set(ended.asks.filter((one) => one.plane === plane).map((one) => one.key));
  const inIt = (one: InboxUpdate) =>
    one.at >= ended.from && one.at <= ended.to && !one.read && isNew(updateSeenKey(one.key));
  return {
    asks: (asks ?? []).filter((ask) => came.has(ask.ask) && isNew(askSeenKey(ask))),
    finished: (updates ?? []).filter((one) => one.kind === "task-done" && inIt(one)),
    failed: (updates ?? []).filter((one) => one.kind === "task-failed" && inIt(one)),
  };
}

/** One part of a summary: which, and its words. */
export type QuietPart = { part: keyof QuietSummaryOf; says: string };

/** **A summary's parts, in words**, those with nothing left out: "3 asks", "2 finished". */
export function quietPartsSaid(summary: QuietSummaryOf): QuietPart[] {
  const parts: QuietPart[] = [];
  const { asks, finished, failed } = summary;
  if (asks.length > 0)
    parts.push({ part: "asks", says: `${asks.length} ${asks.length === 1 ? "ask" : "asks"}` });
  if (finished.length > 0) parts.push({ part: "finished", says: `${finished.length} finished` });
  if (failed.length > 0) parts.push({ part: "failed", says: `${failed.length} failed` });
  return parts;
}

/** The whole summary in one sentence, as a screen reader hears it; empty when nothing is left. */
export function quietSaid(summary: QuietSummaryOf): string {
  const parts = quietPartsSaid(summary);
  return parts.length === 0
    ? ""
    : `While you were quiet: ${parts.map((one) => one.says).join(", ")}`;
}
