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
      <div data-scope="button-row">
        <Button>Later</Button>
      </div>
      <div data-scope="notice" data-part="under">
        <Button>Copy</Button>
      </div>
    </div>
  );
}

describe("a secondary button on a notice card (#3478)", () => {
  it("is drawn a step toward the words on the card's row of ways, and a further step pointed at", () => {
    render(<OnACard layout="card" />);
    const button = screen.getByRole("button", { name: "Later" });
    expect(button).toHaveAttribute("data-variant", "secondary");
    expect(drawnWith(button, "background-color")).toBe(
      "color-mix(in oklab, var(--control-base), var(--text-primary) 14%)",
    );
    expect(drawnWith(button, "background-color", { hover: true })).toBe(
      "color-mix(in oklab, var(--control-base), var(--text-primary) 22%)",
    );
  });

  it("is the family's own in what the card opens under its ways, and off a card", () => {
    render(<OnACard layout="card" />);
    expect(drawnWith(screen.getByRole("button", { name: "Copy" }), "background-color")).toBe(
      "var(--control-base)",
    );
    cleanup();
    render(<OnACard />);
    expect(drawnWith(screen.getByRole("button", { name: "Later" }), "background-color")).toBe(
      "var(--control-base)",
    );
  });
});
