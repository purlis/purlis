// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { RESET, writePerFrame, type Write } from "./paneWrites";

/** A terminal that records what it was asked to write and parses only when told to. */
function terminal() {
  const written: string[] = [];
  const unparsed: (() => void)[] = [];
  const write: Write = (bytes, parsed) => {
    written.push(new TextDecoder().decode(bytes));
    if (parsed) unparsed.push(parsed);
  };
  const parse = () => unparsed.splice(0).forEach((then) => then());
  return { written, unparsed, write, parse };
}

/** `text` as the bytes a program writes. */
const bytes = (text: string) => new TextEncoder().encode(text);

/** Frames the test runs by hand. */
function frames() {
  let due: (() => void) | undefined;
  return {
    next: (then: () => void) => {
      due = then;
      return () => {
        due = undefined;
      };
    },
    run: () => {
      const then = due;
      due = undefined;
      then?.();
    },
  };
}

describe("a pane's output, gathered per frame (#4732)", () => {
  it("tells each chunk's waiter, in order, once the one write carrying it is parsed", () => {
    const into = terminal();
    const clock = frames();
    const writes = writePerFrame(into.write, clock.next);
    const told: string[] = [];

    writes.add(bytes("a"), () => told.push("a"));
    writes.add(bytes("b"));
    writes.add(bytes("c"), () => told.push("c"));
    clock.run();
    expect(into.written).toEqual(["abc"]);
    expect(told).toEqual([]);

    into.parse();

    expect(told).toEqual(["a", "c"]);
  });

  it("hands the terminal no callback for a write nobody waits on", () => {
    const into = terminal();
    const clock = frames();
    const writes = writePerFrame(into.write, clock.next);

    writes.add(bytes("plain"));
    clock.run();

    expect(into.written).toEqual(["plain"]);
    expect(into.unparsed).toEqual([]);
  });

  it("asks for one frame however many chunks arrive before it", () => {
    let asked = 0;
    const writes = writePerFrame(
      () => {},
      () => {
        asked += 1;
        return () => {};
      },
    );

    writes.add(bytes("x"));
    writes.add(bytes("y"));
    writes.add(bytes("z"));

    expect(asked).toBe(1);
  });

  it("drops what it gathered before a reset, and still tells those chunks drawn (#4810)", () => {
    // Frames stop while the window is hidden; a view that fell behind comes back at its next
    // epoch, a reset ahead of a fresh screen. What came before is never drawn, and is acked.
    const into = terminal();
    const clock = frames();
    const writes = writePerFrame(into.write, clock.next);
    const told: string[] = [];

    writes.add(bytes("old 1"), () => told.push("old 1"));
    writes.add(bytes("old 2"), () => told.push("old 2"));
    writes.add(RESET, () => told.push("reset"));
    writes.add(bytes("fresh"), () => told.push("fresh"));
    clock.run();
    expect(into.written).toEqual(["\x1bcfresh"]);

    into.parse();

    expect(told).toEqual(["old 1", "old 2", "reset", "fresh"]);
  });
});
