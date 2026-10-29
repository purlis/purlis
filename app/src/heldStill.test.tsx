// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { createPortal } from "react-dom";
import { useHeldStill } from "./heldStill";

/**
 * **The hold a list keeps still by** (V100-47), drawn small: one row, the row's menu drawn in a
 * layer of its own as a context menu is, and a button and a menu of the window's elsewhere.
 */
function Held({
  on = true,
  menu = false,
  wait = 0,
}: {
  on?: boolean;
  menu?: boolean;
  /** How long the list asks the pointer's hold to wait as the pointer leaves it. */
  wait?: number;
}) {
  const { holding, held } = useHeldStill<HTMLDivElement>({}, on, () => wait);
  return (
    <>
      <p data-testid="holding">{String(holding)}</p>
      <div {...held}>
        <button type="button">row</button>
        {menu &&
          createPortal(
            <div role="menu" aria-label="The row's menu">
              <button type="button" role="menuitem">
                Open
              </button>
            </div>,
            document.body,
          )}
      </div>
      <button type="button">elsewhere</button>
      <div role="menu" aria-label="Another menu">
        <button type="button" role="menuitem">
          Other
        </button>
      </div>
    </>
  );
}

const holding = () => screen.getByTestId("holding").textContent === "true";
const button = (name: string) => screen.getByRole("button", { name });
const item = (name: string) => screen.getByRole("menuitem", { name });
/** The keyboard comes to the row: a key pressed on it. */
const keyboardOnTheRow = () => {
  act(() => button("row").focus());
  fireEvent.keyDown(button("row"), { key: "ArrowDown" });
};

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

/** The pointer goes from `from` onto `to`, as the browser says it. */
const pointerFromTo = (from: Element, to: Element) =>
  fireEvent.pointerOver(to, { relatedTarget: from });

describe("a list held still (useHeldStill)", () => {
  it("is held while the keyboard is in a row's own menu, drawn in a layer of its own", () => {
    render(<Held menu />);
    keyboardOnTheRow();
    expect(holding()).toBe(true);

    act(() => item("Open").focus());
    expect(holding()).toBe(true);

    act(() => button("elsewhere").focus());
    expect(holding()).toBe(false);
  });

  it("lets go at once when the keyboard goes from a row to a menu that is not the list's", () => {
    render(<Held menu />);
    keyboardOnTheRow();
    expect(holding()).toBe(true);

    act(() => item("Other").focus());

    expect(holding()).toBe(false);
  });

  it("lets the pointer's hold go once the pointer is over something else after a row's menu went from under it", () => {
    const { rerender } = render(<Held menu />);
    // The pointer comes onto the row's menu from outside the list.
    fireEvent.pointerOver(item("Open"), { relatedTarget: null });
    expect(holding()).toBe(true);

    // An item is chosen and the menu goes from under the pointer, which is outside the list:
    // what it is over next is said with no element it came from, so no leave is heard.
    rerender(<Held />);
    fireEvent.pointerOver(button("elsewhere"), { relatedTarget: null });

    expect(holding()).toBe(false);
  });

  it("lets the pointer's hold go once the pointer is over a menu that is not the list's", () => {
    const { rerender } = render(<Held menu />);
    fireEvent.pointerOver(item("Open"), { relatedTarget: null });
    expect(holding()).toBe(true);

    // The row's menu went from under the pointer, which comes next onto another menu.
    rerender(<Held />);
    fireEvent.pointerOver(item("Other"), { relatedTarget: null });

    expect(holding()).toBe(false);
  });

  it("keeps the pointer's hold while the pointer moves from one of its elements to another", () => {
    render(<Held menu />);
    fireEvent.pointerOver(button("row"), { relatedTarget: null });
    fireEvent.pointerOver(item("Open"), { relatedTarget: button("row") });

    expect(holding()).toBe(true);
  });

  it("lets go of what it held when it is turned off, and is not held when it is turned on again", () => {
    const { rerender } = render(<Held />);
    fireEvent.pointerOver(button("row"), { relatedTarget: null });
    expect(holding()).toBe(true);

    rerender(<Held on={false} />);
    expect(holding()).toBe(false);

    rerender(<Held />);
    expect(holding()).toBe(false);
  });

  it("lets the pointer's hold go at once as the pointer leaves, where its list asks no wait", () => {
    render(<Held />);
    fireEvent.pointerOver(button("row"), { relatedTarget: null });
    expect(holding()).toBe(true);

    pointerFromTo(button("row"), button("elsewhere"));

    expect(holding()).toBe(false);
  });

  it("keeps the pointer's hold through the wait its list asks, and on when the pointer comes back over the list's own in that time", () => {
    vi.useFakeTimers();
    render(<Held menu wait={150} />);
    fireEvent.pointerOver(button("row"), { relatedTarget: null });

    // Off the list, as onto the gap beside a row's card: held while the wait lasts.
    pointerFromTo(button("row"), button("elsewhere"));
    act(() => void vi.advanceTimersByTime(100));
    expect(holding()).toBe(true);

    // Onto what the list draws in a layer of its own, as a row's card is: still held, for good.
    pointerFromTo(button("elsewhere"), item("Open"));
    act(() => void vi.advanceTimersByTime(1000));
    expect(holding()).toBe(true);
  });

  it("lets the pointer's hold go once the wait its list asks is over, with the pointer elsewhere", () => {
    vi.useFakeTimers();
    render(<Held wait={150} />);
    fireEvent.pointerOver(button("row"), { relatedTarget: null });

    pointerFromTo(button("row"), button("elsewhere"));
    act(() => void vi.advanceTimersByTime(149));
    expect(holding()).toBe(true);
    act(() => void vi.advanceTimersByTime(1));

    expect(holding()).toBe(false);
  });

  it("lets go at once, wait or none, when the window stops being the one in use", () => {
    vi.useFakeTimers();
    render(<Held wait={150} />);
    fireEvent.pointerOver(button("row"), { relatedTarget: null });
    pointerFromTo(button("row"), button("elsewhere"));

    act(() => void window.dispatchEvent(new Event("blur")));

    expect(holding()).toBe(false);
  });
});
