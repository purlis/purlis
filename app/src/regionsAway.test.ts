// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { DEFAULT_ARRANGEMENT, PANEL, viewMoved, type Arrangement } from "./regions";
import { putAway } from "./regionsAway";

/**
 * **What the palette calls put away** (#1752, train 64's review): the regions collapsed, and the
 * Panel while no half of it is drawn now. A half holding only extensions' panels that are not
 * contributed now is not drawn, so it does not count as the Panel shown.
 */
describe("the regions put away, for the palette", () => {
  const BURN = "panel:ext/stats/burn" as const;
  /** The left half put away, and the right half holding only an extension's panel. */
  const onlyAPanel = (): Arrangement => {
    let moved: Arrangement = DEFAULT_ARRANGEMENT;
    for (const view of ["memory", "todos", "sessions"] as const)
      moved = viewMoved(moved, view, "aside");
    moved = viewMoved(moved, BURN, "panel-right");
    return moved.map((one) => (one.id === "panel-left" ? { ...one, collapsed: true } : one));
  };

  it("names nothing while every region and the Panel are drawn", () => {
    expect(putAway(DEFAULT_ARRANGEMENT, [])).toEqual([]);
  });

  it("names the Panel away while its only half left holds a panel not contributed now", () => {
    expect(putAway(onlyAPanel(), [])).toEqual(["panel-left", PANEL]);
  });

  it("names the Panel shown once that panel is contributed again", () => {
    expect(putAway(onlyAPanel(), [BURN])).toEqual(["panel-left"]);
  });

  it("names each collapsed region in the arrangement's order", () => {
    const both = DEFAULT_ARRANGEMENT.map((one) =>
      one.id === "aside" || one.id === "navigation" ? { ...one, collapsed: true } : one,
    );
    expect(putAway(both, [])).toEqual(["navigation", "aside"]);
  });
});
