// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { writePerFrame, type Write } from "./paneWrites";

/** A terminal that records what it was asked to write and parses only when told to. */
function terminal() {
  const written: string[] = [];
  const unparsed: (() => void)[] = [];
  const write: Write = (text, parsed) => {
    written.push(text);
    if (parsed) unparsed.push(parsed);
  };
  const parse = () => unparsed.splice(0).forEach((then) => then());
  return { written, unparsed, write, parse };
}

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

    writes.add("a", () => told.push("a"));
    writes.add("b");
    writes.add("c", () => told.push("c"));
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

    writes.add("plain");
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

    writes.add("x");
    writes.add("y");
    writes.add("z");

    expect(asked).toBe(1);
  });
});
