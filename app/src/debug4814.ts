// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// [DEBUG-4814] Temporary: a trace of the picker's events and the core's asks, read by a failing
// scenario spec (`e2e/opening.ts`). Removed once the macOS picker race is found.

type Event = { t: number; what: string };
const events: Event[] = [];
const t0 = performance.now();

/** [DEBUG-4814] Writes down one event, keeping the last 200. */
export function debug4814(what: string): void {
  events.push({ t: Math.round(performance.now() - t0), what });
  if (events.length > 200) events.shift();
}

(globalThis as { __debug4814?: () => readonly Event[] }).__debug4814 = () => events;
