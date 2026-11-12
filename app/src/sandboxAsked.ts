// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useSyncExternalStore } from "react";
import { listen, untilStopped } from "./here";

/**
 * **The window changed what a chat's sandbox is compiled from** (#1428, D-1428-10): one of its
 * own sandbox commands returned, such as a folder every chat may write being revoked in
 * Settings' Granted list, a folder listed or unlisted, a host added, removed or confirmed, a
 * block allowed past this chat, or the sandbox's offer answered.
 *
 * What those write is kept in the project's state folder, which the watcher of the project's
 * root does not report. So the window says so itself, and whatever reads the chats left on an
 * older sandbox (`useOlderSandbox`) asks again. A count and not an event: a reader that mounts
 * later still sees that it moved.
 */
let asked = 0;
const listeners = new Set<() => void>();

/** One of the window's sandbox commands returned: anything that follows it reads again. */
export function sandboxCommandReturned(): void {
  asked += 1;
  for (const listener of [...listeners]) listener();
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => void listeners.delete(listener);
}

/** Calls `listener` each time one of the window's sandbox commands returns; answers its stop. */
export const onSandboxCommand = (listener: () => void): (() => void) => subscribe(listener);

/** How many of the window's sandbox commands have returned: it moves when one does. */
export function useSandboxCommands(): number {
  return useSyncExternalStore(
    subscribe,
    () => asked,
    () => asked,
  );
}

/**
 * **The chats' Run grants moved** (#5617): a run's notice was answered, whose Allow gives one, or
 * Keep blocked was pressed on a vault, which takes back the chat's grants on it. The core keeps
 * them in memory and tells nobody, so the window says so itself, and the Allowed list reads
 * again. A count, as {@link sandboxCommandReturned}'s is.
 */
let grants = 0;
const grantListeners = new Set<() => void>();

/** The event the core's word that a project's Run grants moved arrives on (#5800). */
export const RUN_GRANTS_MOVED = "run-grants-moved";

/** A press that gives or takes back a Run grant returned: the Allowed list reads again. */
export function runGrantsMoved(): void {
  grants += 1;
  for (const listener of [...grantListeners]) listener();
}

/**
 * How many presses that give or take back a Run grant have returned, and how often the core said
 * a project's Run grants moved some other way (#5800): an Allow once its run took or a check
 * dropped, or the grants of a chat that closed. Any project's word reads the list again.
 */
export function useRunGrantsMoved(): number {
  useEffect(() => untilStopped(listen(RUN_GRANTS_MOVED, runGrantsMoved)), []);
  return useSyncExternalStore(
    (listener) => {
      grantListeners.add(listener);
      return () => void grantListeners.delete(listener);
    },
    () => grants,
    () => grants,
  );
}
