// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A pane typed into and sent nothing back has a stuck view** (#5478).
 *
 * A chat's pane froze half-drawn while its person typed, and stayed so, though its program ran
 * on. A tab switch kept it frozen, since the chat's terminal and view are kept
 * (`keptTerminals.ts`); falling out of the kept chats cured it at once, since that lets go of
 * both and starts them again. So the stuck part is the terminal and view pair, and most likely
 * the view: the host sends a pane no more than a watermark ahead of what it reports drawn
 * (#4733), so a pane whose reports fall short is sent nothing ever again.
 *
 * A program always writes back what is typed to it: a shell's line discipline echoes it, a
 * harness draws it in its input box. So a pane that typed text at least {@link LEAST_TYPED}
 * times and was sent **nothing at all** for {@link NO_ECHO_MS}, while the window was visible
 * and the terminal in it, is told to start again. What a program may answer with nothing (a
 * mouse or focus report, a lone Escape, anything that starts with one) does not count as typed.
 * Output arriving is what counts, not what xterm drew of it, so a hidden window's frames, which
 * stop, never look stuck here.
 *
 * Starting again is what falling out of the kept chats does, so it costs what that costs: the
 * screen is sent again. The program, and what is typed to it, are untouched, and the new
 * terminal takes the keyboard the old one had. So a prompt that echoes nothing (a password
 * typed over more than {@link NO_ECHO_MS}), or a program busy that long, may start it again
 * for nothing, and its person loses no key to it. A chat starts again at most once per
 * {@link AT_MOST_EVERY_MS}.
 *
 * #5478's cause was in xterm itself (`altScreenRows.ts`); this stays as the fallback that keeps a
 * person's chat working should a view stick for any other reason.
 */

/** How long a pane may type and be sent nothing before its view is taken to be stuck. */
export const NO_ECHO_MS = 10_000;

/** How many times something must be typed, in that time, before it says anything. */
export const LEAST_TYPED = 2;

/** The least time between two starts again of one chat. */
export const AT_MOST_EVERY_MS = 5 * 60_000;

/** How often the watch looks. */
const LOOK_EVERY_MS = 1_000;

/** What the watch is told by the pane. */
export interface NoEchoWatch {
  /** Input went to the program, as xterm's `onData` gave it. */
  typed(data: string): void;
  /** Output arrived from the program, for the view the pane draws. */
  output(): void;
  /** The pane is gone. */
  dispose(): void;
}

/** What the watch saw of a stuck view. */
export interface NoEcho {
  typed: number;
  waitedMs: number;
}

/** Whether `data` is text typed, which a program writes back, rather than a report or a key it
 *  may answer with nothing. */
const isTyped = (data: string) => data.length > 0 && !data.includes("\x1b");

/** When each chat last started again, by its key (`keptTerminals.keptKey`). */
const started = new Map<string, number>();

/** A test's: forgets every start again. */
export function forgetStartsAgain(): void {
  started.clear();
}

/**
 * Watches one pane's terminal for a stuck view, and calls `stuck` once if it sees one and chat
 * `key` may start again now. `connected` says whether the terminal is in the window.
 */
export function watchForNoEcho(
  key: string,
  connected: () => boolean,
  stuck: (said: NoEcho) => void,
  page: { readonly visibilityState: DocumentVisibilityState } = document,
): NoEchoWatch {
  let since: number | undefined;
  let typed = 0;
  let told = false;

  const look = () => {
    if (since === undefined || told) return;
    if (page.visibilityState !== "visible" || !connected()) {
      since = undefined;
      typed = 0;
      return;
    }
    const now = Date.now();
    if (now - since < NO_ECHO_MS || typed < LEAST_TYPED) return;
    const last = started.get(key);
    if (last !== undefined && now - last < AT_MOST_EVERY_MS) {
      since = undefined;
      typed = 0;
      return;
    }
    started.set(key, now);
    told = true;
    stuck({ typed, waitedMs: now - since });
  };
  const timer = setInterval(look, LOOK_EVERY_MS);

  return {
    typed(data) {
      if (!isTyped(data)) return;
      since ??= Date.now();
      typed += 1;
    },
    output() {
      since = undefined;
      typed = 0;
    },
    dispose() {
      clearInterval(timer);
    },
  };
}
