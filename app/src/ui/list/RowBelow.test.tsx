// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A line under a row's words, cut short, said whole in the row's tooltip** (#4711): a
 * `RowBelow` that asks for it says its whole words in its row's tooltip while its clamp cuts
 * them, under the pointer and while the keyboard is on the row, and says nothing while they fit.
 * jsdom draws nothing, so whether the line is cut is stubbed here; the stories measure it in a
 * real browser (`List.stories.tsx`).
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { TOOLTIP_DELAY } from "../tooltip/Tooltip";
import { List, ListRow } from "./List";
import { RowBelow, RowLabel } from "./Row";

const LONG =
  "Left out: a name starting with purlis- is purlis's own, so this one is never handed on";
const SHORT = "Left out: off here";

/** The lines under whose words are wider than their box, as a browser would draw them. */
let cut: Set<string>;

beforeEach(() => {
  cut = new Set();
  vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockImplementation(function (
    this: HTMLElement,
  ) {
    return cut.has(this.textContent ?? "") ? 400 : 100;
  });
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockImplementation(() => 100);
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

function Skills({ whole = true }: { whole?: boolean }) {
  return (
    <List aria-label="Skills">
      <ListRow id="long" textValue="long">
        <RowLabel>long</RowLabel>
        <RowBelow clamp={1} tipWhenCut={whole}>
          {LONG}
        </RowBelow>
      </ListRow>
      <ListRow id="short" textValue="short">
        <RowLabel>short</RowLabel>
        <RowBelow clamp={1} tipWhenCut={whole}>
          {SHORT}
        </RowBelow>
      </ListRow>
    </List>
  );
}

const row = (name: string) => screen.getByRole("row", { name: new RegExp(`^${name}`) });

function hover(name: string) {
  fireEvent.pointerEnter(row(name), { pointerType: "mouse" });
  act(() => void vi.advanceTimersByTime(TOOLTIP_DELAY));
}

describe("a line under a row's words that asks for its whole in the tooltip (#4711)", () => {
  it("says its whole words under the pointer while its clamp cuts them", () => {
    vi.useFakeTimers();
    cut.add(LONG);
    render(<Skills />);
    hover("long");
    expect(screen.getByRole("tooltip")).toHaveTextContent(LONG);
  });

  it("says its whole words while the keyboard is on its row", async () => {
    cut.add(LONG);
    render(<Skills />);
    await userEvent.tab();
    expect(row("long")).toHaveFocus();
    expect(within(document.body).getByRole("tooltip")).toHaveTextContent(LONG);
  });

  it("never describes its row, whose own words already say all of it", async () => {
    cut.add(LONG);
    render(<Skills />);
    await userEvent.tab();
    expect(screen.getByRole("tooltip")).toBeInTheDocument();
    expect(row("long")).not.toHaveAttribute("aria-describedby");
  });

  it("shows no tooltip while its words fit", async () => {
    vi.useFakeTimers();
    cut.add(LONG);
    render(<Skills />);
    hover("short");
    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  it("shows none on a row whose line does not ask, however it is cut", () => {
    vi.useFakeTimers();
    cut.add(LONG);
    render(<Skills whole={false} />);
    hover("long");
    expect(screen.queryByRole("tooltip")).toBeNull();
  });
});
