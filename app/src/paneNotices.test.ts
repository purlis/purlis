// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  bandHeadSays,
  bandRegionSays,
  bellCount,
  bellSays,
  dismissAllSays,
  foldedSays,
} from "./paneNotices";
import { loadPaneNotices, PANE_NOTICES } from "./paneNoticesPref";

describe("a pane's notices, minimised or expanded (#4754)", () => {
  it("is minimised where the layout file says nothing of it", () => {
    expect(loadPaneNotices({})).toEqual({ value: "minimised", said: [] });
    expect(loadPaneNotices(null)).toEqual({ value: "minimised", said: [] });
  });

  it("reads expanded back, and writes only expanded, which minimised leaves out", () => {
    expect(loadPaneNotices({ notices: "expanded" })).toEqual({ value: "expanded", said: [] });
    PANE_NOTICES.forget();
    PANE_NOTICES.keep("expanded");
    expect(PANE_NOTICES.written()).toBe("expanded");
    PANE_NOTICES.keep("minimised");
    expect(PANE_NOTICES.written()).toBeUndefined();
    PANE_NOTICES.forget();
  });

  it("says what it put right when the file holds something else, and minimises", () => {
    const loaded = loadPaneNotices({ notices: 3 });
    expect(loaded.value).toBe("minimised");
    expect(loaded.said).toEqual([
      '"notices" 3 is not "minimised" or "expanded", so a pane\'s notices are minimised',
    ]);
  });
});

describe("what a pane's bell says", () => {
  it("counts the notes and never the asks, and is empty with no notes", () => {
    expect(bellCount({ notes: 5, trouble: 1, asks: 2 })).toBe("5");
    expect(bellCount({ notes: 0, trouble: 0, asks: 2 })).toBe("");
  });

  it("says the notes, the trouble and the asks to a screen reader", () => {
    expect(bellSays({ notes: 5, trouble: 1, asks: 2 })).toBe("5 notices, 1 in trouble, 2 asking");
    expect(bellSays({ notes: 1, trouble: 0, asks: 0 })).toBe("1 notice");
    expect(bellSays({ notes: 0, trouble: 0, asks: 1 })).toBe("No notices, 1 asking");
  });

  it("heads the open band with what waits on you, then its notes", () => {
    expect(bandHeadSays({ notes: 5, trouble: 1, asks: 2 })).toBe("2 waiting on you · 5 notices");
    expect(bandHeadSays({ notes: 0, trouble: 0, asks: 3 })).toBe("3 waiting on you");
    expect(bandHeadSays({ notes: 1, trouble: 0, asks: 0 })).toBe("1 notice");
  });

  it("names the band's region with its count", () => {
    expect(bandRegionSays({ notes: 0, trouble: 0, asks: 3 })).toBe(
      "Pane notices: 3 waiting on you",
    );
  });

  it("folds to how many wait and on what, each once", () => {
    expect(foldedSays(3, ["Remove workspace", "Remove workspace", "Terminal prompt"])).toBe(
      "3 waiting · Remove workspace, Terminal prompt",
    );
    expect(foldedSays(1, [])).toBe("1 waiting");
  });

  it("counts what Dismiss all clears", () => {
    expect(dismissAllSays(4)).toBe("Dismiss all 4");
  });
});
