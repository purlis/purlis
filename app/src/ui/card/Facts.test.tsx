// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Facts and totals** (#3673, the card family): a list of facts a reader hears as label and
 * value, a row of tiles, and live totals that say their changes politely at most once a second.
 * The look itself is held by the stories (`Card.stories.tsx`).
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, within } from "@testing-library/react";
import { drawnWith } from "../../cascade.testkit";
import { SAID_AT_MOST_EVERY } from "../said";
import { Badge } from "../badge/Badge";
import { Button } from "../button/Button";
import { Fact, Facts } from "./Facts";
import { Tile, Totals } from "./Totals";

afterEach(cleanup);

describe("Facts", () => {
  it("is a list of terms, each label with its value", () => {
    render(
      <Facts aria-label="About deploy">
        <Fact label="Owner">you</Fact>
        <Fact label="Grade">
          86% with it <Badge>sonnet</Badge>
        </Fact>
        <Fact label="Who gets it">
          Left out of steward 5 <Button size="sm">Grant it</Button>
        </Fact>
      </Facts>,
    );
    const list = screen.getAllByRole("term")[0].closest("dl");
    expect(list).toHaveAttribute("aria-label", "About deploy");
    const terms = screen.getAllByRole("term").map((one) => one.textContent);
    const values = screen.getAllByRole("definition");
    expect(terms).toEqual(["Owner", "Grade", "Who gets it"]);
    expect(values[0]).toHaveTextContent("you");
    // A value may hold a badge and a fix.
    expect(within(values[2]).getByRole("button", { name: "Grant it" })).toBeVisible();
    // Each label and its value are one group of the list.
    expect(terms.length).toBe(values.length);
    expect(values[1].parentElement).toBe(screen.getAllByRole("term")[1].parentElement);
  });

  it("draws its label in the secondary tier, its value in the primary", () => {
    render(
      <Facts>
        <Fact label="Owner">you</Fact>
      </Facts>,
    );
    expect(drawnWith(screen.getByRole("term"), "color")).toBe("var(--text-secondary)");
    expect(drawnWith(screen.getByRole("definition"), "color")).toBe("var(--text-primary)");
  });
});

describe("Totals", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("is a group of tiles, each its label, its value in figures of one width and its line", () => {
    render(
      <Totals aria-label="What it may spend">
        <Tile label="Chats" value="36" line="12 tests × 3 × 1 arm" />
        <Tile label="At most" value="$3.00" />
      </Totals>,
    );
    const group = screen.getByRole("group", { name: "What it may spend" });
    const value = within(group).getByText("36");
    expect(value).toHaveAttribute("data-part", "tile-value");
    expect(drawnWith(value, "font-variant-numeric")).toBe("tabular-nums");
    expect(within(group).getByText("12 tests × 3 × 1 arm")).toHaveAttribute(
      "data-part",
      "tile-line",
    );
    // Not live: nothing is said as it changes.
    expect(within(group).queryByRole("status")).toBeNull();
  });

  it("says its changes politely, at most once a second, and nothing as it is drawn", () => {
    const row = (done: number, spent: string) => (
      <Totals live aria-label="Grading">
        <Tile label="Chats" value={`${done} of 12`} />
        <Tile label="Spent" value={spent} line="of at most $3.00" />
      </Totals>
    );
    const { rerender } = render(row(0, "$0.00"));
    act(() => vi.advanceTimersByTime(0));
    const status = screen.getByRole("status");
    expect(status).toHaveTextContent("");
    rerender(row(1, "$0.08"));
    act(() => vi.advanceTimersByTime(0));
    expect(status).toHaveTextContent("Chats 1 of 12, Spent $0.08");
    rerender(row(2, "$0.16"));
    act(() => vi.advanceTimersByTime(SAID_AT_MOST_EVERY / 4));
    rerender(row(3, "$0.24"));
    act(() => vi.advanceTimersByTime(SAID_AT_MOST_EVERY / 4));
    // Still within the second: the last words said stay.
    expect(status).toHaveTextContent("Chats 1 of 12, Spent $0.08");
    act(() => vi.advanceTimersByTime(SAID_AT_MOST_EVERY / 2));
    // Only the newest is said, once.
    expect(status).toHaveTextContent("Chats 3 of 12, Spent $0.24");
  });
});
