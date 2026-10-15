// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { frameBytes, latestOnly } from "./browserFrames";

/** A draw the test finishes by hand. */
function slowDraw() {
  const drawn: number[] = [];
  const finish: (() => void)[] = [];
  const draw = (frame: Uint8Array) =>
    new Promise<void>((done) => {
      drawn.push(frame[0]);
      finish.push(done);
    });
  return { drawn, finish, draw };
}

describe("a browser view's frames (#2012)", () => {
  it("are read back from the base64 the window is sent", () => {
    expect(Array.from(frameBytes("/9j/AA=="))).toEqual([0xff, 0xd8, 0xff, 0x00]);
  });

  it("are drawn one at a time, and a view that falls behind draws only the newest", async () => {
    const { drawn, finish, draw } = slowDraw();
    const show = latestOnly(draw);

    show(Uint8Array.of(1));
    show(Uint8Array.of(2));
    show(Uint8Array.of(3));
    expect(drawn).toEqual([1]);

    finish.shift()?.();
    await Promise.resolve();
    await Promise.resolve();
    expect(drawn).toEqual([1, 3]);

    finish.shift()?.();
    await Promise.resolve();
    await Promise.resolve();
    expect(drawn).toEqual([1, 3]);
  });

  it("go on being drawn after one that could not be decoded", async () => {
    const drawn: number[] = [];
    const show = latestOnly(async (frame) => {
      drawn.push(frame[0]);
      if (frame[0] === 1) throw new Error("not a picture");
    });
    show(Uint8Array.of(1));
    await Promise.resolve();
    await Promise.resolve();
    show(Uint8Array.of(2));
    await Promise.resolve();
    expect(drawn).toEqual([1, 2]);
  });
});
