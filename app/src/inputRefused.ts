// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useSyncExternalStore } from "react";
import { listen } from "./here";
import type { InputRefused, PlaneId } from "./bindings";

/**
 * **What a chat was not sent, as its pane says it** (#4734, ADR 0108). A pane's keys, paste,
 * resizes and view go to the session host in the chat's own lane, off the window's thread, so a
 * refusal comes after the ask has answered: the core tells it (`chat-input-refused`), once for
 * each run of sends it dropped, and the pane holds the newest until it is dismissed.
 */

const EVENT = "chat-input-refused";

let held: Readonly<Record<string, string>> = {};
const listeners = new Set<() => void>();
const key = (plane: PlaneId, session: number) => `${plane}\0${session}`;

function set(next: Readonly<Record<string, string>>) {
  held = next;
  for (const one of listeners) one();
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** The core said a send to `told.session` was refused, and why. */
export function heardInputRefused(told: InputRefused) {
  set({ ...held, [key(told.plane, told.session)]: told.why });
}

/** Chat `session`'s refusal is put away: dismissed, or the pane's test is over. */
export function forgetInputRefused(plane: PlaneId, session: number) {
  const at = key(plane, session);
  if (!(at in held)) return;
  set(Object.fromEntries(Object.entries(held).filter(([one]) => one !== at)));
}

/** Why the newest send to chat `session` was refused, until it is dismissed. */
export function useInputRefused(plane: PlaneId, session: number): string | undefined {
  useEffect(() => {
    let gone = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await listen<InputRefused>(EVENT, (event) => {
          if (event.payload.plane === plane && event.payload.session === session) {
            heardInputRefused(event.payload);
          }
        });
        if (gone) unlisten();
        else stop = unlisten;
      } catch {
        // No window to listen in: a unit test, or a webview being torn down.
      }
    })();
    return () => {
      gone = true;
      stop?.();
    };
  }, [plane, session]);
  return useSyncExternalStore(subscribe, () => held[key(plane, session)]);
}
