// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";
import { commands } from "./bindings";

/**
 * **What each project switch in this window took, written down where the person can read it**
 * (the doctor's dialog, `Doctor.tsx`).
 *
 * The operator reported a freeze of one to three seconds on a switch. What a test and CI's
 * runner can measure is not his window on his machine, with his projects and his chats, so the
 * window times itself, and he can copy what it found:
 *
 * - **drawn**: from the press that made the switch to the window having drawn it, which is
 *   React's commit and whatever was laid out for it;
 * - **painted**: from the press to the engine having painted that;
 * - **each synchronous command the core ran for it**, from the ask to the answer. A synchronous
 *   command is answered on the window's own thread in the core (`off_the_main_thread.rs`), which
 *   pumps the window's events: while one runs, the window does nothing else, a paint included.
 *   `sandbox_state` is the one worth watching: it starts `git` where the project has a local
 *   settings file.
 *
 * Each is also a `performance.measure` (`purlis:switch:…`, `purlis:command:…`), for an inspector
 * where there is one. Kept for this window only, the last {@link KEPT} switches, in memory:
 * nothing is written anywhere and nothing is sent.
 */

/** How many switches are kept. */
export const KEPT = 20;

/** How long after a switch a command still counts as made for it. */
export const AFTER_MS = 2_000;

/** One command the core was asked while a switch was being drawn, or just after. */
export type Asked = { command: string; ms: number; afterPaint: boolean };

/** One switch. */
export type Switched = {
  /** The project that came to the front. */
  to: string;
  /** When, by the clock on the wall. */
  at: number;
  /** The press to the window having drawn it; nothing where no press made the switch. */
  drawnMs: number | undefined;
  /** The press, or the switch itself where there was none, to the paint. */
  paintedMs: number | undefined;
  asked: Asked[];
};

/**
 * The synchronous commands a switch can make, by what the core calls them and the window's name
 * for each: every one `ipc_commands.rs` lists without `async` that a project's view, or the
 * window, asks as a project comes to the front.
 */
const SYNCHRONOUS = {
  sandbox_state: "sandboxState",
  sandbox_kept_blocked: "sandboxKeptBlocked",
  inbox_shown: "inboxShown",
  window_holds_planes: "windowHoldsPlanes",
  tasks_sharing_a_folder: "tasksSharingAFolder",
  persona_grants_held: "personaGrantsHeld",
  chat_usage: "chatUsage",
} as const satisfies Record<string, keyof typeof commands>;

let switches: readonly Switched[] = [];
/** The switch commands are counted for now, and since when on the window's own clock. */
let latest: { switched: Switched; since: number; at: number; painted: boolean } | undefined;
const listeners = new Set<() => void>();

function changed(): void {
  switches = [...switches];
  for (const listener of listeners) listener();
}

function measure(name: string, start: number, end: number): void {
  try {
    performance.measure(name, { start, end });
  } catch {
    // An engine without it, or a start it will not take: the list above has the number.
  }
}

/**
 * A project came to the front: called once the window has drawn that (a layout effect), with
 * the event that made the switch where one did. The press is that event's own time, so what is
 * timed is what the person waited, and not what was left of it by the time React got there.
 */
export function switched(to: string, by: Event | undefined = globalThis.event): void {
  const now = performance.now();
  // An event's time is on the same clock, and never after now; one from long ago is the last
  // thing the person did, and not this switch.
  const pressed =
    by !== undefined && by.timeStamp > 0 && now - by.timeStamp < AFTER_MS
      ? by.timeStamp
      : undefined;
  // The same project again within a breath is the same switch drawn twice, as React's strict
  // mode draws everything in a development build.
  if (latest !== undefined && latest.switched.to === to && now - latest.at < 5) return;
  const one: Switched = {
    to,
    at: Date.now(),
    drawnMs: pressed === undefined ? undefined : now - pressed,
    paintedMs: undefined,
    asked: [],
  };
  const mine = { switched: one, since: pressed ?? now, at: now, painted: false };
  latest = mine;
  switches = [...switches, one].slice(-KEPT);
  if (pressed !== undefined) measure("purlis:switch:drawn", pressed, now);
  changed();
  // The frame being drawn now, then the task after it: that frame has been painted.
  requestAnimationFrame(() =>
    setTimeout(() => {
      const then = performance.now();
      one.paintedMs = then - mine.since;
      mine.painted = true;
      measure("purlis:switch:painted", mine.since, then);
      changed();
    }, 0),
  );
}

/** Times `ask`, and counts it for the switch it was made in. */
function timed<A extends unknown[], R>(command: string, ask: (...args: A) => Promise<R>) {
  return (...args: A): Promise<R> => {
    const from = performance.now();
    const mine = latest;
    const afterPaint = mine?.painted ?? false;
    const answered = () => {
      const now = performance.now();
      measure(`purlis:command:${command}`, from, now);
      if (mine === undefined || from - mine.since > AFTER_MS) return;
      mine.switched.asked.push({ command, ms: now - from, afterPaint });
      changed();
    };
    return ask(...args).finally(answered);
  };
}

let started = false;

/**
 * Starts timing the synchronous commands, once, for the life of the window: each is asked
 * through the same function as before, which now writes down how long its answer took.
 */
export function startTheSwitchClock(): void {
  if (started) return;
  started = true;
  const asks = commands as unknown as Record<string, (...args: unknown[]) => Promise<unknown>>;
  for (const [command, name] of Object.entries(SYNCHRONOUS))
    asks[name] = timed(command, asks[name].bind(commands));
}

/** Forgets every switch: a test's window is a new one. */
export function forgetSwitches(): void {
  switches = [];
  latest = undefined;
}

/** The switches kept, oldest first. */
export function useSwitches(): readonly Switched[] {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => switches,
  );
}

const ms = (n: number | undefined) => (n === undefined ? "not timed" : `${Math.round(n)} ms`);

/**
 * One switch as a line a person can read and copy: when, to which project, how long to drawn
 * and to painted, and each synchronous command by how often it was asked and how long its
 * answers took in all, the ones asked before the paint apart from the ones after it.
 */
export function said(one: Switched, nameOf: (plane: string) => string = (plane) => plane): string {
  const when = new Date(one.at).toLocaleTimeString([], { hour12: false });
  const sum = (asked: readonly Asked[]) => {
    const by = new Map<string, { times: number; ms: number; longest: number }>();
    for (const { command, ms: took } of asked) {
      const of = by.get(command) ?? { times: 0, ms: 0, longest: 0 };
      by.set(command, {
        times: of.times + 1,
        ms: of.ms + took,
        longest: Math.max(of.longest, took),
      });
    }
    return [...by]
      .sort(([, a], [, b]) => b.ms - a.ms)
      .map(
        ([command, of]) =>
          `${command} ×${of.times} ${Math.round(of.ms)} ms` +
          (of.times > 1 ? ` (longest ${Math.round(of.longest)})` : ""),
      )
      .join(", ");
  };
  const before = sum(one.asked.filter((asked) => !asked.afterPaint));
  const after = sum(one.asked.filter((asked) => asked.afterPaint));
  return (
    `${when} to ${nameOf(one.to)}: drawn ${ms(one.drawnMs)}, painted ${ms(one.paintedMs)}` +
    `; the core's synchronous commands before the paint: ${before || "none"}` +
    `; after it: ${after || "none"}`
  );
}
