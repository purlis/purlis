// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { catalogue, type Now, type Offer } from "./actions";
import { PANE_FLOOR } from "./paneDrop";
import { noTabs, openTab, type Tabs } from "./tabs";

/**
 * **Split right and Split down below the minimum, as rows of the one list** (#4766, spec #4750
 * story 84): the palette's rows grey out with their reason where the focused pane's half would
 * be under about 40 × 8, each by its own axis, and neither turns into the other.
 */

function now(tabs: Tabs, over: Partial<Now> = {}): Now {
  return { tabs, workspaces: [], needsYou: [], nameOf: (session) => String(session), ...over };
}

const row = (offers: readonly Offer[], id: string): Offer | undefined =>
  offers.find((offer) => offer.id === id);

const one = () => openTab(noTabs(), 7, "7", "steward");

describe("the split rows below the minimum (#4766)", () => {
  it("grey Split right with its reason on a pane too narrow to halve", () => {
    const offers = catalogue(
      now(one(), { paneRoom: { width: 2 * PANE_FLOOR.width - 1, height: 600 } }),
    );

    expect(row(offers, "pane.split.right")).toMatchObject({
      available: false,
      reason:
        "Too narrow to split: each side would be under about 40 columns. Widen the window or close a pane.",
    });
    expect(row(offers, "pane.split.down")).toMatchObject({
      available: true,
      does: { verb: "split", direction: "column" },
    });
  });

  it("grey Split down with its reason on a pane too short to halve", () => {
    const offers = catalogue(
      now(one(), { paneRoom: { width: 1200, height: 2 * PANE_FLOOR.height - 1 } }),
    );

    expect(row(offers, "pane.split.down")?.available).toBe(false);
    expect(row(offers, "pane.split.right")).toMatchObject({
      available: true,
      does: { verb: "split", direction: "row" },
    });
  });

  it("offer both where the pane is not measured", () => {
    const offers = catalogue(now(one()));

    expect(row(offers, "pane.split.right")?.available).toBe(true);
    expect(row(offers, "pane.split.down")?.available).toBe(true);
  });
});
