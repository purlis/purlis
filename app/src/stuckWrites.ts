// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Write } from "./paneWrites";

/**
 * **A terminal whose writes stop being parsed is started again** (#5478).
 *
 * xterm parses what it is written in a loop of its own, on timers, and an exception thrown in
 * it (`WriteBuffer._innerWrite` has no `try`) ends that loop for good: everything written after
 * waits for ever, its callbacks are never called, and the pane never reports it drawn. The
 * session host sends a view no more than a watermark ahead of what it reports (#4733), so the
 * host stops sending too, and the pane stays half-drawn, its keys and mouse going nowhere it
 * shows. `altScreenRows.ts` closes the one way xterm.js 6.0.0 was seen to throw there; this is
 * for any other.
 *
 * A write is watched until xterm says it is parsed. One that throws where the pane can see it
 * (xterm parses at once the first write after a key is typed), or one still unparsed after
 * {@link UNPARSED_MS} while the window is visible, says the terminal is stuck. xterm parses a
 * view's watermark of output in milliseconds, so that wait is never a busy terminal; a hidden
 * window's timers may be slowed, so the wait starts again when it is shown.
 *
 * Starting again is what falling out of the kept chats does: the terminal is disposed, its
 * view closed, and a new one opened and sent the screen as it is now. A chat starts again for
 * this at most once per {@link AT_MOST_EVERY_MS}.
 */

/** How long a write may wait to be parsed, the window visible, before the terminal is stuck. */
export const UNPARSED_MS = 5_000;

/** The least time between two starts again of one terminal for a stuck write. */
export const AT_MOST_EVERY_MS = 30_000;

/** How often the watch looks. */
const LOOK_EVERY_MS = 1_000;

/** What the watch saw of a stuck terminal. */
export type StuckWrite =
  { by: "threw"; error: string } | { by: "unparsed"; waitedMs: number; unparsed: number };

/** A terminal's writes, watched. */
export interface WatchedWrites {
  /** Writes as `write` does, watched until parsed. */
  write: Write;
  /** The terminal is gone. */
  dispose(): void;
}

/** When each terminal last started again for a stuck write, by its chat's key. */
const started = new Map<string, number>();

/** A test's: forgets every start again. */
export function forgetStuckWrites(): void {
  started.clear();
}

/**
 * Watches the writes chat `key`'s terminal is given through `write`, and calls `stuck` once if
 * the terminal stops parsing them and the chat may start again now.
 */
export function watchWrites(
  key: string,
  write: Write,
  stuck: (said: StuckWrite) => void,
  page: { readonly visibilityState: DocumentVisibilityState } = document,
): WatchedWrites {
  /** When each write still unparsed was made, or the window last shown, oldest first. */
  const waiting: number[] = [];
  let told = false;

  const tell = (said: StuckWrite) => {
    if (told) return;
    const now = Date.now();
    const last = started.get(key);
    if (last !== undefined && now - last < AT_MOST_EVERY_MS) return;
    started.set(key, now);
    told = true;
    stuck(said);
  };

  const look = () => {
    if (told || waiting.length === 0) return;
    const now = Date.now();
    if (page.visibilityState !== "visible") {
      waiting.fill(now);
      return;
    }
    const waitedMs = now - waiting[0];
    if (waitedMs >= UNPARSED_MS) tell({ by: "unparsed", waitedMs, unparsed: waiting.length });
  };
  const timer = setInterval(look, LOOK_EVERY_MS);

  return {
    write(bytes, parsed) {
      waiting.push(Date.now());
      try {
        write(bytes, () => {
          waiting.shift();
          parsed?.();
        });
      } catch (err) {
        tell({
          by: "threw",
          error: err instanceof Error ? `${err.name}: ${err.message}` : String(err),
        });
      }
    },
    dispose() {
      clearInterval(timer);
    },
  };
}
