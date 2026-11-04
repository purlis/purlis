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
/** How many times a line's width was asked for: each is a layout the browser has to do. */
let measured = 0;

beforeEach(() => {
  cut = new Set();
  measured = 0;
  vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockImplementation(function (
    this: HTMLElement,
  ) {
    measured += 1;
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

  it("is its row's description while it is shown, the row being named by its words alone", async () => {
    cut.add(LONG);
    render(<Skills />);
    await userEvent.tab();
    expect(row("long")).toHaveAccessibleName("long");
    expect(row("long")).toHaveAccessibleDescription(LONG);
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

  it("is its row's description under the pointer too, while it is shown", () => {
    vi.useFakeTimers();
    cut.add(LONG);
    render(<Skills />);
    hover("long");
    expect(row("long")).toHaveAccessibleName("long");
    expect(row("long")).toHaveAccessibleDescription(LONG);
  });
});

describe("whether a line is cut, measured only when its row could say so (#4351)", () => {
  it("measures no line while the rows are only drawn", () => {
    cut.add(LONG);
    render(<Skills />);
    expect(measured).toBe(0);
  });

  it("says the whole of a line cut after its row was drawn, as the pointer comes", () => {
    vi.useFakeTimers();
    render(<Skills />);
    // The row grew narrower since: the line is cut now, and the row was told nothing.
    cut.add(LONG);
    hover("long");
    expect(screen.getByRole("tooltip")).toHaveTextContent(LONG);
  });

  it("says the whole of a line cut after its row was drawn, as the keyboard comes", async () => {
    render(<Skills />);
    cut.add(LONG);
    await userEvent.tab();
    expect(row("long")).toHaveFocus();
    expect(screen.getByRole("tooltip")).toHaveTextContent(LONG);
    expect(row("long")).toHaveAccessibleDescription(LONG);
  });

  it("measures nothing for a pointer that passes over the rows", () => {
    vi.useFakeTimers();
    cut.add(LONG);
    render(<Skills />);
    fireEvent.pointerEnter(row("long"), { pointerType: "mouse" });
    fireEvent.pointerLeave(row("long"), { pointerType: "mouse" });
    fireEvent.pointerEnter(row("short"), { pointerType: "mouse" });
    fireEvent.pointerLeave(row("short"), { pointerType: "mouse" });
    act(() => void vi.advanceTimersByTime(TOOLTIP_DELAY));
    expect(measured).toBe(0);
  });

  it("says the new whole words of a line whose words change while the keyboard is on it", async () => {
    const { rerender } = render(<OneLine words={SHORT} />);
    await userEvent.tab();
    expect(screen.queryByRole("tooltip")).toBeNull();

    cut.add(LONG);
    rerender(<OneLine words={LONG} />);

    expect(screen.getByRole("tooltip")).toHaveTextContent(LONG);
    expect(row("long")).toHaveAccessibleDescription(LONG);
  });

  it("takes its tooltip down once the line it was for fits, the keyboard still on the row", async () => {
    cut.add(LONG);
    const { rerender } = render(<OneLine words={LONG} />);
    await userEvent.tab();
    expect(screen.getByRole("tooltip")).toHaveTextContent(LONG);

    rerender(<OneLine words={SHORT} />);

    expect(screen.queryByRole("tooltip")).toBeNull();
  });
});

/** One row whose line says `words`. */
function OneLine({ words }: { words: string }) {
  return (
    <List aria-label="Skills">
      <ListRow id="long" textValue="long">
        <RowLabel>long</RowLabel>
        <RowBelow clamp={1} tipWhenCut>
          {words}
        </RowBelow>
      </ListRow>
    </List>
  );
}
