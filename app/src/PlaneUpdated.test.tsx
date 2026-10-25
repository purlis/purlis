// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Offer } from "./actions";
import { FreshMark } from "./PlaneUpdated";

const FRESH: Offer = {
  id: "tab.fresh:3",
  title: "Start fresh",
  available: true,
  reason: "",
  does: { verb: "startFresh", tab: 3 },
};

/**
 * **The fresh mark on the strip, a button of the family** (NO-3, #1246, #3129): out of the Tab
 * order, as the strip is one stop, and the tab beside it is described by it, through its `id`.
 */
describe("the fresh mark", () => {
  it("is no Tab stop, and its tab is described by its name", () => {
    render(
      <>
        <button type="button" role="tab" aria-selected aria-describedby="fresh-3">
          steward 3
        </button>
        <FreshMark id="fresh-3" offer={FRESH} files={["CLAUDE.md"]} onPress={() => undefined} />
      </>,
    );
    const name = "Start fresh — project updated since this chat started";
    const mark = screen.getByRole("button", { name });
    expect(mark.tabIndex).toBe(-1);
    // The tab's description is the mark, read through its id; what a browser reads from it is its
    // name, its `aria-label` (jsdom's description reads only a described element's words, and the
    // mark has none but its icon, so the reference is held, and the name it leads to).
    const tab = screen.getByRole("tab", { name: "steward 3" });
    const described = document.getElementById(tab.getAttribute("aria-describedby") ?? "");
    expect(described).toBe(mark);
    expect(described).toHaveAccessibleName(name);
  });
});
