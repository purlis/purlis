// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  listen as listenAnywhere,
  type EventCallback,
  type UnlistenFn,
} from "@tauri-apps/api/event";
import { eventHandled } from "./slowLog";

/**
 * This window's own name, and the one way the page listens for the core's events (charter#126).
 *
 * **Every listener names this window as its target.** `@tauri-apps/api/event`'s `listen`
 * defaults to the target `Any`, and Tauri delivers every event to an `Any` listener, including
 * one the core sent to a different window with `emit_to`. A second window listening that way
 * would take in a project moved to the main window, answer the main window's quit, and open a
 * second launch's directory as well. A listener that names its own window still hears what the
 * core sends to every window (`emit`), and hears `emit_to` only when it is addressed here.
 */

type Internals = { metadata?: { currentWindow?: { label?: string } } };

/** The main window's label: the one `tauri.conf.json` declares. */
export const MAIN = "main";

/**
 * This window's label, as Tauri gave it — the main window's when the page is not in a Tauri
 * window at all (a unit test), which is the window every test before split windows was about.
 *
 * Read from the metadata Tauri writes into every page before it loads, which is what
 * `getCurrentWindow()` reads too; that call throws outside a Tauri window rather than answering.
 */
export function thisWindow(): string {
  const internals = (globalThis as { __TAURI_INTERNALS__?: Internals }).__TAURI_INTERNALS__;
  return internals?.metadata?.currentWindow?.label ?? MAIN;
}

/** Listens for `event` in this window: what the core sends every window, and what it sends
 *  this one. Never what it sends another window. Its stop never fails: see {@link quietly}.
 *  Each one handled is counted for the frame it is handled in, so a stall can name it
 *  (`slowLog.ts`). */
export function listen<T>(event: string, handler: EventCallback<T>): Promise<UnlistenFn> {
  const counted: EventCallback<T> = (heard) => {
    eventHandled(event);
    return handler(heard);
  };
  return listenAnywhere<T>(event, counted, { target: thisWindow() }).then(quietly);
}

/**
 * `unlisten`, made a stop that never fails (#6194). Tauri's unlisten is `async`, so once the
 * window's event bridge is gone (the window closing, or a test's mock torn down) it rejects
 * where no `try` around the call can catch it, and the rejection is unhandled. Nothing is
 * listening any more by then, which is what the stop was for.
 */
function quietly(unlisten: UnlistenFn): UnlistenFn {
  return () => {
    try {
      void Promise.resolve(unlisten() as unknown).catch(() => {});
    } catch {
      // The bridge went before the call: nothing is listening.
    }
  };
}

/**
 * **A listen held until the returned stop is called**, the shape an effect's cleanup wants
 * (#6194). The stop may come before `listening` answers: the listener is then stopped as it
 * answers, so a component gone first leaves nothing listening. Neither stop ever fails.
 */
export function untilStopped(listening: Promise<UnlistenFn>): () => void {
  let stop: UnlistenFn | undefined;
  let gone = false;
  listening.then(
    (unlisten) => {
      if (gone) quietly(unlisten)();
      else stop = quietly(unlisten);
    },
    () => {},
  );
  return () => {
    gone = true;
    stop?.();
  };
}
