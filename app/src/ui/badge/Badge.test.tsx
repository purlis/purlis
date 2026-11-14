// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { drawnWith } from "../../cascade.testkit";
import { Tooltip } from "../tooltip/Tooltip";
import { Badge, Count, CountGroup, HostStatus, RoundCount, capped } from "./Badge";

/** `value`, which the story has drawn: it fails here, by name, where it is not. */
function drawn<T>(value: T | null | undefined, what: string): T {
  if (value === null || value === undefined) throw new Error(`${what} is not drawn`);
  return value;
}

/**
 * **The badge and count family** (#2912): a word on a pill and a number on a pill, drawn by their
 * parts, a count heard as what it counts, and a badge with a hint reached by the keyboard.
 */

afterEach(cleanup);

describe("Badge", () => {
  it("is the family's root in its variant, its look the family's", () => {
    render(<Badge variant="waiting">deprecated</Badge>);
    const badge = screen.getByText("deprecated");
    expect(badge).toHaveAttribute("data-scope", "badge");
    expect(badge).toHaveAttribute("data-part", "root");
    expect(badge).toHaveAttribute("data-variant", "waiting");
    expect(drawnWith(badge, "background-color")).toBe("var(--wash-warning)");
    expect(drawnWith(badge, "color")).toBe("var(--state-waiting)");
    expect(drawnWith(badge, "border-radius")).toBe("var(--radius-full)");
    expect(drawnWith(badge, "font-size")).toBe("var(--font-size-xs)");
  });

  it("says Your hosts' two statuses in their own words (R66)", () => {
    render(
      <>
        <HostStatus waiting />
        <HostStatus waiting={false} />
      </>,
    );
    expect(screen.getByText("Waiting for you")).toHaveAttribute("data-variant", "waiting");
    expect(drawnWith(screen.getByText("Confirmed"), "color")).toBe("var(--state-success)");
  });

  it("is a Tab stop when it has a hint, and only then", async () => {
    const user = userEvent.setup();
    render(
      <>
        <Tooltip tip="Set on this machine only.">
          <Badge variant="accent">This machine</Badge>
        </Tooltip>
        <Badge>purlis project</Badge>
      </>,
    );
    await user.tab();
    expect(screen.getByText("This machine")).toHaveFocus();
    expect(screen.getByText("purlis project")).not.toHaveAttribute("tabindex");
  });
});

describe("Count", () => {
  it("draws its number and is heard as what it counts", () => {
    render(
      <Count variant="hot" said="2 chats need you" data-testid="count">
        2
      </Count>,
    );
    const count = screen.getByTestId("count");
    expect(count).toHaveAttribute("data-scope", "count");
    expect(count).toHaveAttribute("data-variant", "hot");
    expect(screen.getByText("2")).toHaveAttribute("aria-hidden", "true");
    expect(screen.getByText("2 chats need you")).toHaveClass("sr-only");
    expect(drawnWith(count, "background-color")).toBe("var(--needs-you-base)");
    expect(drawnWith(count, "color")).toBe("var(--needs-you-text)");
  });

  it("draws a small count, to stand on a mark's corner, at its own least size (#5189)", () => {
    render(
      <>
        <Count data-testid="small" size="sm" aria-hidden="true">
          3
        </Count>
        <Count data-testid="plain" aria-hidden="true">
          3
        </Count>
      </>,
    );
    const small = screen.getByTestId("small");
    expect(small).toHaveAttribute("data-size", "sm");
    expect(screen.getByTestId("plain")).not.toHaveAttribute("data-size");
    expect(drawnWith(small, "min-block-size")).toBe("calc(12rem / 14)");
    expect(drawnWith(small, "min-inline-size")).toBe("calc(12rem / 14)");
    expect(drawnWith(small, "padding-inline")).toBe("var(--space-1)");
    expect(drawnWith(small, "line-height")).toBe("1");
    expect(drawnWith(screen.getByTestId("plain"), "min-block-size")).toBe("calc(16rem / 14)");
  });

  it("draws counts as one pill in a group, square between them", () => {
    render(
      <CountGroup role="img" aria-label="3 chats, 1 needs you">
        <Count aria-hidden="true">3</Count>
        <Count variant="hot" aria-hidden="true">
          1
        </Count>
      </CountGroup>,
    );
    const group = screen.getByRole("img", { name: "3 chats, 1 needs you" });
    expect(group).toHaveAttribute("data-part", "group");
    expect(drawnWith(group, "border-radius")).toBe("var(--radius-full)");
    expect(drawnWith(group.firstElementChild as Element, "border-radius")).toBe("0");
  });
});

describe("RoundCount", () => {
  const round = (container: HTMLElement) =>
    container.querySelector<HTMLElement>('[data-scope="count"][data-part="round"]');

  it("draws 1 to 9 as they are and 10 or more as 9+, one character's circle", () => {
    expect([1, 9, 10, 120].map(capped)).toEqual(["1", "9", "9+", "9+"]);
    const { container, rerender } = render(<RoundCount count={9} at="mark" />);
    expect(round(container)).toHaveAttribute("data-drawn", "9");
    rerender(<RoundCount count={120} at="mark" />);
    expect(round(container)).toHaveAttribute("data-drawn", "9+");
    const circle = drawn(round(container), "the count");
    expect(drawnWith(circle, "inline-size")).toBe("16px");
    expect(drawnWith(circle, "block-size")).toBe("16px");
    expect(drawnWith(circle, "background-color")).toBe("var(--needs-you-count)");
    expect(drawnWith(circle, "color")).toBe("var(--needs-you-count-text)");
  });

  it("is 14px inline after words, and nothing at all for 0", () => {
    const { container, rerender } = render(<RoundCount count={3} at="inline" said="3" />);
    expect(drawnWith(drawn(round(container), "the count"), "inline-size")).toBe("14px");
    rerender(<RoundCount count={0} at="inline" said="0" />);
    expect(round(container)).toBeNull();
  });

  it("is heard as the real number, never 9+", () => {
    render(<RoundCount count={120} at="inline" said="120" />);
    expect(screen.getByText("120")).toBeInTheDocument();
    expect(screen.queryByText("9+")).toBeNull();
  });

  it("is hidden where its host says the number", () => {
    const { container } = render(<RoundCount count={4} at="mark" />);
    expect(round(container)).toHaveAttribute("aria-hidden", "true");
  });

  it("slides once on each change of what it draws, the same element each time", () => {
    const { container, rerender } = render(<RoundCount count={2} at="mark" />);
    const first = round(container);
    expect(first).not.toHaveAttribute("data-slide");
    rerender(<RoundCount count={3} at="mark" />);
    expect(round(container)).toHaveAttribute("data-slide", "a");
    expect(round(container)).toHaveAttribute("data-slide-dir", "up");
    rerender(<RoundCount count={1} at="mark" />);
    expect(round(container)).toHaveAttribute("data-slide", "b");
    expect(round(container)).toHaveAttribute("data-slide-dir", "down");
    expect(round(container)).toBe(first);
  });

  it("does not slide when what it draws is the same (9+ to 9+)", () => {
    const { container, rerender } = render(<RoundCount count={9} at="mark" />);
    rerender(<RoundCount count={12} at="mark" />);
    expect(round(container)).toHaveAttribute("data-slide", "a");
    rerender(<RoundCount count={40} at="mark" />);
    expect(round(container)).toHaveAttribute("data-slide", "a");
    expect(round(container)).toHaveAttribute("data-slide-dir", "up");
  });
});
