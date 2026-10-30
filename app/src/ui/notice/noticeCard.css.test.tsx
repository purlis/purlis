// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A secondary button on a notice card is visibly a button** (#3478): the card stands on the
 * overlay ground, and in Purlis a secondary button's own fill (`control.base`) is that same
 * colour, so "Add a vision" read as words with no box. On a card, a secondary button is drawn a
 * step off its fill, toward the words, at rest and under the pointer, in every theme; a theme's
 * own `button.root` fill, in the `parts` layer above, still wins.
 *
 * vitest applies no stylesheet, so what a rule draws is asked of the cascade over the window's
 * sheets (`cascade.testkit.ts`), on the markup the wrappers write.
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { drawnWith } from "../../cascade.testkit";
import { Button } from "../button/Button";

afterEach(cleanup);

function OnACard({ layout }: { layout?: "card" }) {
  return (
    <div data-scope="notice" data-part="root" data-layout={layout} data-placement="alone">
      <Button>Add a vision</Button>
    </div>
  );
}

describe("a secondary button on a notice card (#3478)", () => {
  it("stands apart from the card's ground, at rest and under the pointer", () => {
    render(<OnACard layout="card" />);
    const button = screen.getByRole("button", { name: "Add a vision" });
    expect(button).toHaveAttribute("data-variant", "secondary");
    const rest = drawnWith(button, "background-color");
    const hover = drawnWith(button, "background-color", { hover: true });
    expect(rest).toBe("var(--control-hover)");
    expect(hover).toBe("var(--control-aimed)");
  });

  it("is the family's own elsewhere", () => {
    render(<OnACard />);
    const button = screen.getByRole("button", { name: "Add a vision" });
    expect(drawnWith(button, "background-color")).toBe("var(--control-base)");
  });
});
