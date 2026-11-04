// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Where a frame is scrolled to show a focused control and its ring** (#4672): the same place
 * from the same start, whatever the browser's own focus scroll did in between.
 */
import { describe, expect, it } from "vitest";
import { revealedScrollLeft } from "./reveal";

const frame = { room: 400, most: 600, ring: 4 };

describe("revealedScrollLeft", () => {
  it("leaves the frame where it stood when the control and its ring are in sight", () => {
    expect(revealedScrollLeft({ ...frame, start: 0, left: 100, right: 180 })).toBe(0);
    expect(revealedScrollLeft({ ...frame, start: 50, left: 54, right: 446 })).toBe(50);
  });

  it("brings a control past the right edge in with its ring inside the edge", () => {
    expect(revealedScrollLeft({ ...frame, start: 0, left: 500, right: 580 })).toBe(184);
  });

  it("brings a control whose ring alone is cut at the right edge in by the ring", () => {
    expect(revealedScrollLeft({ ...frame, start: 0, left: 300, right: 398 })).toBe(2);
  });

  it("brings a control past the left edge in with its ring inside the edge", () => {
    expect(revealedScrollLeft({ ...frame, start: 300, left: 100, right: 180 })).toBe(96);
  });

  it("never scrolls past either end", () => {
    expect(revealedScrollLeft({ ...frame, start: 10, left: 2, right: 50 })).toBe(0);
    expect(revealedScrollLeft({ ...frame, start: 0, left: 950, right: 998 })).toBe(600);
  });

  it("takes no room for a ring drawn inside the control", () => {
    expect(revealedScrollLeft({ ...frame, ring: 0, start: 0, left: 500, right: 580 })).toBe(180);
  });
});
