// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * What the core writes a view, as a piece of its own, before the screen of its next epoch
 * (`hostedsessions.rs`): `ESC c`.
 */
export const RESET = new Uint8Array([0x1b, 0x63]);

/** Whether `bytes` is a whole {@link RESET} piece. */
const isReset = (bytes: Uint8Array) =>
  bytes.length === RESET.length && bytes.every((byte, at) => byte === RESET[at]);

/**
 * What a pane writes to its terminal: xterm's `write`, the bytes its program wrote and a
 * callback once they are parsed. The terminal decodes them (#4745).
 */
export type Write = (bytes: Uint8Array, parsed?: () => void) => void;

/** A pane's output, gathered until the next frame. */
export interface FrameWrites {
  /** Bytes the view sent, and what to call once they are in the terminal's grid. */
  add(bytes: Uint8Array, parsed?: () => void): void;
  /** Drops what is gathered and asks for no more frames: the pane is gone. */
  dispose(): void;
}

/**
 * **One terminal write per pane per frame** (#4732). A harness streaming a long message sends
 * it as many chunks of at most 16 KiB; written one by one, each is a pass of xterm's parser and
 * a render it schedules. Gathered here, every chunk that arrives within a frame reaches the
 * terminal as one write, in the order it came. What a chunk's `parsed` waits for is still told,
 * once the write that carries it has been parsed, so a benchmark's text-reached-the-grid timing
 * is unchanged (`bench.paneSent`). A {@link RESET} drops the text gathered before it (#4810).
 */
export function writePerFrame(
  write: Write,
  nextFrame: (then: () => void) => () => void = onNextFrame,
): FrameWrites {
  let chunks: Uint8Array[] = [];
  let parsed: (() => void)[] = [];
  let cancel: (() => void) | undefined;

  const flush = () => {
    cancel = undefined;
    const bytes = joined(chunks);
    const told = parsed;
    chunks = [];
    parsed = [];
    write(bytes, told.length ? () => told.forEach((then) => then()) : undefined);
  };

  return {
    add(bytes, then) {
      // A reset starts the view over from a fresh screen, so what is gathered ahead of it is
      // never drawn: while frames stop (a hidden window) that would be a backlog written in
      // one go on return. Its waiters stay, told with the reset's write, so every piece is
      // still reported drawn and the core's count of them stays exact.
      if (isReset(bytes)) chunks = [];
      chunks.push(bytes);
      if (then) parsed.push(then);
      cancel ??= nextFrame(flush);
    },
    dispose() {
      cancel?.();
      cancel = undefined;
      chunks = [];
      parsed = [];
    },
  };
}

/** `chunks` as one run of bytes, in order: the one chunk itself where there is only one. */
function joined(chunks: Uint8Array[]): Uint8Array {
  if (chunks.length === 1) return chunks[0];
  const all = new Uint8Array(chunks.reduce((length, chunk) => length + chunk.length, 0));
  let at = 0;
  for (const chunk of chunks) {
    all.set(chunk, at);
    at += chunk.length;
  }
  return all;
}

function onNextFrame(then: () => void): () => void {
  const frame = requestAnimationFrame(then);
  return () => cancelAnimationFrame(frame);
}

/** What a pane tells the core of what its terminal has drawn. */
export interface DrawnReports {
  /** One more piece the view sent is in the terminal's grid. */
  piece(): void;
}

/**
 * **What the terminal drew, told back once per write** (#4733). Every piece a write carried is
 * counted as its callback is called, and `report` is told the count once those callbacks are
 * done, in one call rather than one per piece. The core acknowledges the view's bytes to the
 * session host only on this report, so a pane that cannot keep up falls behind where the host
 * sees it, and is sent a fresh screen instead of a backlog.
 */
export function drawnReports(report: (pieces: number) => void): DrawnReports {
  let pieces = 0;
  return {
    piece() {
      pieces += 1;
      if (pieces > 1) return;
      queueMicrotask(() => {
        const told = pieces;
        pieces = 0;
        report(told);
      });
    },
  };
}
