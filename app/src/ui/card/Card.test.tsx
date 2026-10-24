// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { createRef } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { drawnWith } from "../../cascade.testkit";
import { Card, CardTitle } from "./Card";

/** **The card family** (#2912): a box one step off its surface, an ask edged in waiting. */

afterEach(cleanup);

describe("Card", () => {
  it("is the family's root, drawn as the element it is asked for, with its ref", () => {
    const ref = createRef<HTMLElement>();
    render(
      <ul>
        <Card as="li" variant="ask" ref={ref} aria-label="An ask" className="inbox-ask">
          <CardTitle>steward 4 asks</CardTitle>
        </Card>
      </ul>,
    );
    const card = screen.getByRole("listitem", { name: "An ask" });
    expect(ref.current).toBe(card);
    expect(card).toHaveAttribute("data-scope", "card");
    expect(card).toHaveAttribute("data-variant", "ask");
    expect(card).toHaveClass("inbox-ask");
    expect(screen.getByText("steward 4 asks")).toHaveAttribute("data-part", "title");
    expect(drawnWith(card, "border-color")).toBe("var(--state-waiting)");
    expect(drawnWith(card, "border-radius")).toBe("var(--radius-lg)");
  });

  it("draws a raised card on the raised ground with the hairline, and a flush one with no room", () => {
    render(
      <>
        <Card data-testid="raised">x</Card>
        <Card flush data-testid="flush">
          y
        </Card>
      </>,
    );
    const raised = screen.getByTestId("raised");
    expect(drawnWith(raised, "background-color")).toBe("var(--surface-raised)");
    expect(drawnWith(raised, "border")).toBe("1px solid var(--border-subtle)");
    expect(drawnWith(screen.getByTestId("flush"), "padding")).toBe("0");
  });
});
