// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { SLOW_MS, tookWords } from "./took";

describe("tookWords (#7191)", () => {
  it("says a quick check in tenths of a second", () => {
    expect(tookWords(300)).toBe("0.3 s");
    expect(tookWords(1_240)).toBe("1.2 s");
  });
  it("says a slower one in whole seconds, then minutes", () => {
    expect(tookWords(12_400)).toBe("12 s");
    expect(tookWords(64_000)).toBe("1 min 4 s");
    expect(tookWords(180_000)).toBe("3 min");
  });
  it("says nothing where there is no time, or a nonsense one", () => {
    expect(tookWords(null)).toBeUndefined();
    expect(tookWords(-1)).toBeUndefined();
  });
  it("calls a line slow past thirty seconds", () => {
    expect(SLOW_MS).toBe(30_000);
  });
});
