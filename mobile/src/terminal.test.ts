import { describe, expect, it, vi } from "vitest";

import { feed, fromB64, type Drawer } from "./terminal.ts";

describe("fromB64", () => {
  it("decodes the chunk's bytes", () => {
    expect(Array.from(fromB64(btoa("hi\x1b[0m")))).toEqual([104, 105, 27, 91, 48, 109]);
  });
});

describe("feed", () => {
  it("acknowledges the chunk's length once, after it is drawn", () => {
    let drawn: (() => void) | null = null;
    const drawer: Drawer = {
      write(bytes, done) {
        expect(Array.from(bytes)).toEqual([104, 101, 108, 108, 111]);
        drawn = done;
      },
    };
    const ack = vi.fn();
    feed(drawer, btoa("hello"), ack);
    expect(ack).not.toHaveBeenCalled();
    drawn!();
    expect(ack).toHaveBeenCalledWith(5);
    drawn!();
    expect(ack).toHaveBeenCalledTimes(1);
  });

  it("acknowledges an empty chunk as nothing", () => {
    const ack = vi.fn();
    feed({ write: (_bytes, done) => done() }, "", ack);
    expect(ack).toHaveBeenCalledWith(0);
  });
});
