// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { drawnAndMore, movedBy, pinOrderAfterDrop } from "./artifacts";

describe("an artifact group's order (#5953)", () => {
  const ids = ["a", "b", "c", "d"];

  it("moves a pin one place up or down, and says nothing at either end", () => {
    expect(movedBy(ids, "b", -1)).toEqual(["b", "a", "c", "d"]);
    expect(movedBy(ids, "b", 1)).toEqual(["a", "c", "b", "d"]);
    expect(movedBy(ids, "a", -1)).toBeUndefined();
    expect(movedBy(ids, "d", 1)).toBeUndefined();
    expect(movedBy(ids, "zz", 1)).toBeUndefined();
  });

  it("puts a dropped pin where it landed, and says nothing for a drop on itself", () => {
    expect(pinOrderAfterDrop(ids, "a", "c")).toEqual(["b", "c", "a", "d"]);
    expect(pinOrderAfterDrop(ids, "d", "a")).toEqual(["d", "a", "b", "c"]);
    expect(pinOrderAfterDrop(ids, "a", "a")).toBeUndefined();
    expect(pinOrderAfterDrop(ids, "a", "zz")).toBeUndefined();
  });

  it("draws at most eight and keeps every one past them for show-more, none dropped", () => {
    const eleven = Array.from({ length: 11 }, (_, n) => `p${n}`);
    const { drawn, more } = drawnAndMore(eleven);
    expect(drawn).toEqual(eleven.slice(0, 8));
    expect(more).toEqual(eleven.slice(8));
    expect([...drawn, ...more]).toEqual(eleven);
    expect(drawnAndMore(ids)).toEqual({ drawn: ids, more: [] });
  });
});
