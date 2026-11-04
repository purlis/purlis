// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import {
  carryPaneBand,
  forgetPaneBand,
  forgetPaneBands,
  keepHolds,
  keptHolds,
  paneBand,
} from "./noticeSettle";
import { HOLDS_NOTHING } from "./paneNotices";

/**
 * **What a folded pane's rail counts** (#4766): the notes its band held when it was last drawn,
 * kept with the rest of the band's memory, so it goes when the pane closes for good and moves
 * with the pane's Notices.
 */

const three = { notes: 3, trouble: 1, asks: 0 };

afterEach(() => forgetPaneBands());

describe("a folded pane's kept count (#4766)", () => {
  it("is what its band last held", () => {
    keepHolds(paneBand("p", 2), three);

    expect(keptHolds(paneBand("p", 2))).toEqual(three);
  });

  it("is forgotten with the pane's band", () => {
    keepHolds(paneBand("p", 2), three);
    forgetPaneBand(paneBand("p", 2));

    expect(keptHolds(paneBand("p", 2))).toEqual(HOLDS_NOTHING);
  });

  it("is forgotten as the window lets the project go", () => {
    keepHolds(paneBand("p", 2), three);
    keepHolds(paneBand("q", 2), three);
    forgetPaneBands("p");

    expect(keptHolds(paneBand("p", 2))).toEqual(HOLDS_NOTHING);
    expect(keptHolds(paneBand("q", 2))).toEqual(three);
  });

  it("moves with the pane's Notices to a pane that has drawn none", () => {
    keepHolds(paneBand("p", 2), three);
    carryPaneBand(paneBand("p", 2), paneBand("p", 5));

    expect(keptHolds(paneBand("p", 5))).toEqual(three);
  });
});
