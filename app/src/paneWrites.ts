// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** What a pane writes to its terminal: xterm's `write`, text and a callback once it is parsed. */
export type Write = (text: string, parsed?: () => void) => void;

/** A pane's output, gathered until the next frame. */
export interface FrameWrites {
  /** Text the view sent, and what to call once it is in the terminal's grid (a benchmark's). */
  add(text: string, parsed?: () => void): void;
  /** Drops what is gathered and asks for no more frames: the pane is gone. */
  dispose(): void;
}

/**
 * **One terminal write per pane per frame** (#4732). A harness streaming a long message sends
 * it as many chunks of at most 16 KiB; written one by one, each is a pass of xterm's parser and
 * a render it schedules. Gathered here, every chunk that arrives within a frame reaches the
 * terminal as one write, in the order it came. What a chunk's `parsed` waits for is still told,
 * once the write that carries it has been parsed, so a benchmark's text-reached-the-grid timing
 * is unchanged (`bench.paneSent`).
 */
export function writePerFrame(
  write: Write,
  nextFrame: (then: () => void) => () => void = onNextFrame,
): FrameWrites {
  let texts: string[] = [];
  let parsed: (() => void)[] = [];
  let cancel: (() => void) | undefined;

  const flush = () => {
    cancel = undefined;
    const text = texts.join("");
    const told = parsed;
    texts = [];
    parsed = [];
    write(text, told.length ? () => told.forEach((then) => then()) : undefined);
  };

  return {
    add(text, then) {
      texts.push(text);
      if (then) parsed.push(then);
      cancel ??= nextFrame(flush);
    },
    dispose() {
      cancel?.();
      cancel = undefined;
      texts = [];
      parsed = [];
    },
  };
}

function onNextFrame(then: () => void): () => void {
  const frame = requestAnimationFrame(then);
  return () => cancelAnimationFrame(frame);
}
