// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";
import type { PlaneId, RefusedMore } from "./bindings";
import { listen } from "./here";

/**
 * **How many hosts a chat's proxy refused past the ones it names** (#1353): the proxy keeps
 * the first eight hosts it refused, each a block with its own Allow, and counts the rest
 * without their names. The core sends `chat-sandbox-refused-more` each time that count grows,
 * and a host Notice says it, so the ninth host is never hidden.
 *
 * Kept here, for the window's life, rather than on the Notice: a Notice is drawn anew for each
 * host that joins it, and the count may arrive before it is drawn.
 */
export const REFUSED_MORE = "chat-sandbox-refused-more";

const counts = new Map<string, number>();
const watchers = new Set<() => void>();
let listening = false;

const keyOf = (plane: PlaneId, session: number) => `${session}:${plane}`;

/** Takes in one of the core's counts: a count only grows, so an older one arriving late is kept
 *  out. */
export function heardMore(told: RefusedMore) {
  const key = keyOf(told.plane, told.session);
  if ((counts.get(key) ?? 0) >= told.more) return;
  counts.set(key, told.more);
  for (const watcher of watchers) watcher();
}

function listenOnce() {
  if (listening) return;
  listening = true;
  void listen<RefusedMore>(REFUSED_MORE, (event) => heardMore(event.payload)).catch(() => {
    // No window to listen in: a unit test, or a webview being torn down.
    listening = false;
  });
}

function watch(told: () => void) {
  listenOnce();
  watchers.add(told);
  return () => {
    watchers.delete(told);
  };
}

/** How many hosts chat `session`'s proxy refused past the ones it names: 0 for none. */
export function useRefusedMore(plane: PlaneId, session: number): number {
  return useSyncExternalStore(watch, () => counts.get(keyOf(plane, session)) ?? 0);
}

/**
 * What a host Notice says of them: how many, that they are not named, and so that no Allow on
 * it reaches them. Nothing for none.
 */
export function moreRefusedSaid(more: number): string | undefined {
  if (more <= 0) return undefined;
  return more === 1
    ? "1 more host was refused after these, which purlis does not name, so no Allow here reaches it."
    : `${more} more hosts were refused after these, which purlis does not name, so no Allow here reaches them.`;
}
