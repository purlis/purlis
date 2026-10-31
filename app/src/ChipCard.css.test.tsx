// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A notice card holds its notices inside it** (#3653, the probe for #3892): the popover's root
 * is a flex column that React Aria holds to the room on its side, and the card's list is an item
 * of it. Left to its content's height (a flex item's least height), a card stacking many results
 * ran past its root into the page, which lengthened the window by a scrollbar's width
 * (`pane-crumbs.e2e`) and stood over the page's own controls (the Skill studio's New skill…).
 *
 * vitest applies no stylesheet, so what a rule draws is asked of the cascade over the window's
 * sheets (`cascade.testkit.ts`), on the markup `ChipCard` and `HungPopover` write.
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { drawnWith } from "./cascade.testkit";
import { CARD_STOP } from "./regionCycle";

afterEach(cleanup);

describe("a notice card's list (#3653)", () => {
  it("shrinks to the room its card is held to, scrolls inside it, and is capped", () => {
    render(
      <div data-scope="popover" data-part="root" data-variant="notice" className="chip-card">
        <div role="region" aria-label="Notices for one" {...{ [CARD_STOP]: "" }} />
      </div>,
    );
    const root = screen.getByRole("region").parentElement as HTMLElement;
    const list = screen.getByRole("region", { name: "Notices for one" });
    expect(drawnWith(root, "display")).toBe("flex");
    expect(drawnWith(root, "flex-direction")).toBe("column");
    // Never its content's height: a flex item's least height is its content's by default.
    expect(drawnWith(list, "min-block-size")).toBe("0");
    expect(drawnWith(list, "flex")).toBe("0 1 auto");
    expect(drawnWith(list, "overflow-y")).toBe("auto");
    // What its notices place absolutely (a status's words for a screen reader) is placed in it,
    // and clipped with it: placed in the popover instead, a notice far down the list lengthened
    // the page by its own place, out of the list's sight.
    expect(drawnWith(list, "position")).toBe("relative");
    expect(drawnWith(list, "max-block-size")).toBe("min(28rem, 60vh)");
  });
});
