// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  listen as listenAnywhere,
  type EventCallback,
  type UnlistenFn,
} from "@tauri-apps/api/event";
import { listenerCalled } from "./slowLog";

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

type Heard = Parameters<EventCallback<unknown>>[0];

/** One part's handler. Its own object, so a handler given twice is two parts. */
type Part = { handler: EventCallback<unknown> };

/** What the window listens for under one event's name. */
type Listening = {
  parts: Set<Part>;
  /** The core's listener, once it is there: this window's one for the event. */
  listened: Promise<UnlistenFn>;
  /** The bridge and the window it was asked through: one changed is a listener of no use. */
  bridge: unknown;
  target: string;
};

/**
 * **One Tauri listener per event, however many of the window's parts listen for it** (FS-W1,
 * #7909): the window's event hub, by event name. The first part to listen for an event has the
 * core listen for it; each part after joins that listener; the last part's stop has the core stop
 * it. Before it, each part had a listener of its own: 219 for 75 events at 50 chats, each asked
 * for and stopped again as a chat's notices were mounted anew (`windowAsks.cost.test.tsx`).
 */
const listening = new Map<string, Listening>();

/**
 * **The bridge to the core, by the one it registers each listener's callback with.** It is the
 * same for the life of a real window. A stand-in core (`mockIPC`) brings a bridge of its own, and
 * a listener asked through the last one is never called through it: a part listening then is
 * given a listener of the new one's, as each part listened apart before the hub (#7909).
 */
function bridge(): unknown {
  return (globalThis as { __TAURI_INTERNALS__?: { transformCallback?: unknown } })
    .__TAURI_INTERNALS__?.transformCallback;
}

/**
 * Calls each part listening for `event` with what the core said, each though another threw. Each
 * call is counted for the frame it is made in, so a stall can name the event, once, and how many
 * of the window's listeners it called (`slowLog.ts`).
 */
function heardBy(event: string, parts: Set<Part>) {
  return (heard: Heard) => {
    for (const part of [...parts]) {
      // A part stopped by one called before it is not called.
      if (!parts.has(part)) continue;
      listenerCalled(event);
      try {
        void part.handler(heard);
      } catch (error) {
        // Reported as the window reports an error nobody caught (`windowErrors.ts`), which is
        // what a listener's throw was before the hub; the other parts are still called.
        reportError(error);
      }
    }
  };
}

/** The window's listening for `event` now: the one there, or one asked for now. */
function listeningFor(event: string): Listening {
  const target = thisWindow();
  const now = bridge();
  const there = listening.get(event);
  if (there !== undefined && there.bridge === now && there.target === target) return there;
  const parts = new Set<Part>();
  const listened = listenAnywhere<unknown>(event, heardBy(event, parts), { target }).then(quietly);
  const asked: Listening = { parts, listened, bridge: now, target };
  listening.set(event, asked);
  // A refused listen is forgotten, so the next part asks again.
  listened.catch(() => {
    if (listening.get(event) === asked) listening.delete(event);
  });
  return asked;
}

/**
 * The last part of `asked` has stopped: the core's listener is stopped once this task is done,
 * unless a part has listened again by then. A notice mounted again for another chat stops and
 * listens in one commit; it keeps the listener there, and misses nothing said between.
 */
function letGo(event: string, asked: Listening): void {
  queueMicrotask(() => {
    if (asked.parts.size > 0) return;
    if (listening.get(event) === asked) listening.delete(event);
    // A listen still being asked is stopped as it answers.
    asked.listened.then(
      (unlisten) => unlisten(),
      () => {},
    );
  });
}

/** Listens for `event` in this window: what the core sends every window, and what it sends
 *  this one. Never what it sends another window. It answers once the window's one listener for
 *  `event` is there, and its stop never fails: see {@link quietly}. */
export function listen<T>(event: string, handler: EventCallback<T>): Promise<UnlistenFn> {
  const asked = listeningFor(event);
  const part: Part = { handler: handler as EventCallback<unknown> };
  asked.parts.add(part);
  const stop: UnlistenFn = () => {
    if (!asked.parts.delete(part)) return;
    if (asked.parts.size === 0) letGo(event, asked);
  };
  return asked.listened.then(
    () => stop,
    (error: unknown) => {
      asked.parts.delete(part);
      throw error;
    },
  );
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
