// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { placesOf } from "./find";

describe("find in a file's matches (#1088)", () => {
  it("finds the term in any case, overlapping matches too", () => {
    expect(placesOf("Aaa", "aa")).toEqual([
      { from: 0, to: 2 },
      { from: 1, to: 3 },
    ]);
  });

  it("marks the letters of the text itself where lowering would change its length", () => {
    // `İ` lowers to two code units: offsets from a lowered copy would land one letter late.
    const text = "İstanbul and needle";
    const [at] = placesOf(text, "needle");
    expect(text.slice(at.from, at.to)).toBe("needle");
  });

  it("takes the term as text, never as a pattern", () => {
    expect(placesOf("a.b axb (c)", "a.b")).toEqual([{ from: 0, to: 3 }]);
    expect(placesOf("a.b axb (c)", "(c)")).toEqual([{ from: 8, to: 11 }]);
    expect(placesOf("anything", "")).toEqual([]);
  });
});
