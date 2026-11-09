// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * [DEBUG-5478] **Temporary diagnostics for #5478**: a chat's terminal that froze half-drawn,
 * with no typing or mouse, while the chat kept working. The loop in jsdom never froze, so this
 * watches the real app for it and writes down what xterm's state was when it did.
 *
 * A pane is **frozen** here when input was sent to its program, or output arrived from it, and
 * xterm drew nothing after parsing what it was written ({@link Terminal.onRender} once
 * {@link Terminal.onWriteParsed} has caught up) for {@link FROZEN_MS} while the window was
 * visible and the terminal in the page. A hidden window, or a kept terminal no holder shows,
 * draws nothing on purpose, so the clock starts again there. It fires at most once per pane per
 * {@link AT_MOST_EVERY_MS}, and fires one line to the diagnostic log through `debug5478Log`,
 * which the core prefixes `[DEBUG-5478]`.
 *
 * **And looks again {@link FOLLOW_UP_MS} after a line** (#5478, 2026-11-09): one line more, `pane
 * still frozen 30s on`, with the same fields, or `pane recovered after N s` where xterm drew what
 * it was written since. A chat slow to echo recovers; only a `still frozen` line is a freeze.
 *
 * It changes nothing the pane does: it listens, reads xterm's private state when it fires, and
 * wraps xterm's write loop only to note an exception before throwing it on.
 *
 * Everything tagged `[DEBUG-5478]` goes when the freeze is understood, by #5488: one grep.
 */
import type { Terminal } from "@xterm/xterm";

/** [DEBUG-5478] How long a pane may draw nothing after input or output before it is frozen. */
export const FROZEN_MS = 3_000;

/** [DEBUG-5478] The least time between two lines of one pane. */
export const AT_MOST_EVERY_MS = 60_000;

/** [DEBUG-5478] How long after a line the watchdog looks again, to tell slow from frozen. */
export const FOLLOW_UP_MS = 30_000;

/** [DEBUG-5478] How often the watchdog looks. */
const LOOK_EVERY_MS = 1_000;

/** [DEBUG-5478] How many of the write path's exceptions a line carries, the newest. */
const ERRORS_KEPT = 3;

/** [DEBUG-5478] What the watchdog is told by the pane. */
export interface FreezeWatch {
  /** Input went to the program. */
  input(): void;
  /** Output arrived from the program. */
  output(): void;
  /** The pane's frame wrote what it gathered to xterm. */
  written(): void;
  /** `resize_session` was sent with this size. */
  resizeSent(cols: number, rows: number): void;
  /** The write path threw `err`: noted, and the caller throws it on. */
  threw(where: string, err: unknown): void;
  /** The pane is gone. */
  dispose(): void;
}

/** [DEBUG-5478] The watchdog's clock: the browser's own unless a test's. */
export interface Clock {
  now(): number;
  every(ms: number, then: () => void): () => void;
}

const realClock: Clock = {
  now: () => Date.now(),
  every: (ms, then) => {
    const timer = setInterval(then, ms);
    return () => clearInterval(timer);
  },
};

/** [DEBUG-5478] xterm's private parts the line reads, every one of them optional. */
interface Insides {
  _core?: {
    _renderService?: { _isPaused?: boolean };
    _writeBuffer?: {
      _writeBuffer?: unknown[];
      _bufferOffset?: number;
      _pendingData?: number;
      _innerWrite?: (...args: unknown[]) => unknown;
    };
  };
}

/**
 * [DEBUG-5478] Watches `pane` for a freeze, and hands {@link log} one line when it sees one.
 * `session` names the chat in the line. What is watched is the terminal's own element, which
 * moves with it when it is kept (`keptTerminals.ts`), never the holder it was first drawn in.
 */
export function watchForFreeze(
  pane: Terminal,
  session: number,
  log: (line: string) => void,
  clock: Clock = realClock,
  page: { readonly visibilityState: DocumentVisibilityState } = document,
): FreezeWatch {
  let waitingSince: number | undefined;
  let by: "input" | "output" | undefined;
  let inputs = 0;
  let outputs = 0;
  let writes = 0;
  /** Writes handed to xterm and not yet parsed: a draw while any is left is not of them. */
  let unparsed = 0;
  let lastDrawn: number | undefined;
  let lastFired: number | undefined;
  let lastResize: { cols: number; rows: number } | null = null;
  const errors: string[] = [];
  /** Whether the pane is on screen: `null` until the observer says, or where there is none. */
  let intersecting: boolean | null = null;
  /** The line last written, until it is looked at again: when, since when the pane waited,
   *  and when xterm next drew all it was written, if it has. */
  let followUp:
    { firedAt: number; since: number; parsedSince?: boolean; drewAt?: number } | undefined;

  const waiting = (cause: "input" | "output") => {
    if (waitingSince !== undefined) return;
    waitingSince = clock.now();
    by = cause;
    inputs = 0;
    outputs = 0;
    writes = 0;
  };
  // A draw counts only once what was written is parsed: the cursor's blink or a change of
  // focus redraws a row while a stalled write buffer holds everything else back.
  const drew = pane.onRender(() => {
    lastDrawn = clock.now();
    if (unparsed === 0) {
      waitingSince = undefined;
      // Recovered only by drawing output parsed since the line: a focus change redraws a row.
      if (followUp?.parsedSince) followUp.drewAt ??= lastDrawn;
    }
  });
  const parsed = pane.onWriteParsed(() => {
    unparsed = 0;
    if (followUp !== undefined) followUp.parsedSince = true;
  });

  let observer: IntersectionObserver | undefined;
  if (typeof IntersectionObserver !== "undefined") {
    observer = new IntersectionObserver((entries) => {
      const last = entries[entries.length - 1];
      if (last) intersecting = last.isIntersecting;
    });
    if (pane.element) observer.observe(pane.element);
  }

  const threw = (where: string, err: unknown) => {
    const said = err instanceof Error ? `${err.name}: ${err.message}` : String(err);
    errors.push(`${where}: ${said}`.slice(0, 300));
    if (errors.length > ERRORS_KEPT) errors.shift();
  };

  // xterm's write loop goes on in a timer of its own, where an exception would end it for good
  // and reach no caller (`WriteBuffer._innerWrite`, which has no try). It calls itself through
  // `this`, so the wrapper sees every pass. Noted, then thrown on: nothing changes.
  const buffer = (pane as unknown as Insides)._core?._writeBuffer;
  const inner = buffer?._innerWrite;
  if (buffer && typeof inner === "function") {
    buffer._innerWrite = function (this: unknown, ...args: unknown[]) {
      try {
        return inner.apply(this, args);
      } catch (err) {
        threw("_innerWrite", err);
        throw err;
      }
    };
  }

  const line = (now: number): string => {
    const core = (pane as unknown as Insides)._core;
    const written = core?._writeBuffer;
    let synchronizedOutputMode: boolean | null = null;
    try {
      synchronizedOutputMode = pane.modes.synchronizedOutputMode;
    } catch {
      // Read for the line alone: a terminal that cannot say is said as `null`.
    }
    return `pane froze: ${JSON.stringify({
      session,
      by,
      waitedMs: waitingSince === undefined ? null : now - waitingSince,
      inputs,
      outputs,
      writes,
      unparsed,
      lastDrawnMsAgo: lastDrawn === undefined ? null : now - lastDrawn,
      isPaused: core?._renderService?._isPaused ?? null,
      synchronizedOutputMode,
      writeBuffer: {
        length: written?._writeBuffer?.length ?? null,
        offset: written?._bufferOffset ?? null,
        pendingData: written?._pendingData ?? null,
      },
      errors,
      pane: { cols: pane.cols, rows: pane.rows },
      lastResizeSent: lastResize,
      visibilityState: page.visibilityState,
      intersecting,
      connected: pane.element?.isConnected ?? null,
    })}`;
  };

  const write = (said: string) => {
    try {
      log(said);
    } catch {
      // A line that cannot be written is let go: it is a record, never a reason to stop.
    }
  };

  const lookAgain = (now: number) => {
    if (followUp === undefined || now - followUp.firedAt < FOLLOW_UP_MS) return;
    const { since, drewAt } = followUp;
    followUp = undefined;
    if (drewAt !== undefined) {
      const after = Math.round((drewAt - since) / 100) / 10;
      write(
        `pane recovered after ${after} s: ${JSON.stringify({ session, recoveredMs: drewAt - since })}`,
      );
    } else {
      write(
        `pane still frozen ${FOLLOW_UP_MS / 1000}s on: ${line(now).slice("pane froze: ".length)}`,
      );
    }
  };

  const look = () => {
    lookAgain(clock.now());
    if (waitingSince === undefined) return;
    // Nothing is drawn while the window is hidden, or for a kept terminal no holder shows, on
    // purpose. A pane the observer says is off screen is still watched, and the line says so:
    // xterm pauses its renderer on its own observer's word, which may be the freeze itself.
    if (page.visibilityState !== "visible" || pane.element?.isConnected !== true) {
      waitingSince = undefined;
      return;
    }
    const now = clock.now();
    if (now - waitingSince < FROZEN_MS) return;
    if (lastFired !== undefined && now - lastFired < AT_MOST_EVERY_MS) return;
    lastFired = now;
    write(line(now));
    followUp = { firedAt: now, since: waitingSince };
    waitingSince = undefined;
  };
  const stop = clock.every(LOOK_EVERY_MS, look);

  return {
    input() {
      waiting("input");
      inputs += 1;
    },
    output() {
      waiting("output");
      outputs += 1;
    },
    written() {
      writes += 1;
      unparsed += 1;
    },
    resizeSent(cols, rows) {
      lastResize = { cols, rows };
    },
    threw,
    dispose() {
      stop();
      drew.dispose();
      parsed.dispose();
      observer?.disconnect();
      if (buffer && inner) buffer._innerWrite = inner;
    },
  };
}
