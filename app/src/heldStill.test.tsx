// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createPortal } from "react-dom";
import { useEffect, useLayoutEffect, useRef } from "react";
import { useHeldStill, useHeldWhile } from "./heldStill";

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

  it("says on the held element whether it is held, as `data-holding` (#4020)", () => {
    render(<Held />);
    const list = button("row").parentElement as HTMLElement;
    expect(list).not.toHaveAttribute("data-holding");

    fireEvent.pointerOver(button("row"), { relatedTarget: null });
    expect(holding()).toBe(true);
    expect(list).toHaveAttribute("data-holding", "");

    pointerFromTo(button("row"), button("elsewhere"));
    expect(holding()).toBe(false);
    expect(list).not.toHaveAttribute("data-holding");

    keyboardOnTheRow();
    expect(list).toHaveAttribute("data-holding", "");
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

  it("lets go when the pointer leaves the window with no leave on the list (#3095)", async () => {
    render(<Held />);
    fireEvent.pointerEnter(button("row"), { pointerType: "mouse" });
    expect(holding()).toBe(true);
    // WebKit: no `pointerleave` on the list, only an out into nothing.
    await act(async () => void fireEvent.pointerOut(document, { relatedTarget: null }));
    expect(holding()).toBe(false);
  });

  it("keeps the keyboard's hold when the pointer leaves the window (#3095)", async () => {
    render(<Held />);
    keyboardOnTheRow();
    expect(holding()).toBe(true);
    await act(async () => void fireEvent.pointerOut(document, { relatedTarget: null }));
    expect(holding()).toBe(true);
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

/**
 * **A list whose rows the plane changes under the hold**, drawn as the Chats list draws them: a
 * row the plane added while it holds waits for it to let go, and a row the plane took away goes
 * at once. As React Aria does in a browser, the list puts the keyboard back on itself a frame
 * after the row it was on went with it (the browser drops the focus with the row, and React
 * hears no blur). Every draw the window commits writes down whether it was held.
 */
function HeldRows({ rows, frames }: { rows: readonly string[]; frames: boolean[] }) {
  const { holding, held } = useHeldStill<HTMLDivElement>(rows);
  const order = useHeldWhile(holding, rows);
  const drawn = order.filter((row) => rows.includes(row));
  useLayoutEffect(() => {
    frames.push(holding);
  });
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (document.activeElement !== document.body) return;
    const frame = requestAnimationFrame(() => root.current?.focus());
    return () => cancelAnimationFrame(frame);
  }, [rows]);
  return (
    <div {...held}>
      <div ref={root} tabIndex={-1} role="list" aria-label="Chats">
        {drawn.map((row) => (
          <button key={row} type="button" role="listitem">
            {row}
          </button>
        ))}
      </div>
    </div>
  );
}

describe("a list held still while the keyboard is in it (useHeldStill)", () => {
  it("is held through every draw as the row the keyboard is on is taken away, and draws no row that came meanwhile", async () => {
    const frames: boolean[] = [];
    const { rerender } = render(<HeldRows rows={["a", "b"]} frames={frames} />);
    const row = (name: string) => screen.queryByText(name);
    act(() => row("a")?.focus());
    fireEvent.keyDown(row("a") as HTMLElement, { key: "ArrowDown" });
    await waitFor(() => expect(frames.at(-1)).toBe(true));
    // A chat arrives while the keyboard is in the list: held, so not drawn.
    rerender(<HeldRows rows={["a", "b", "c"]} frames={frames} />);
    expect(row("c")).toBeNull();
    const from = frames.length;
    // And the row the keyboard is on is taken away, which sends no blur.
    rerender(<HeldRows rows={["b", "c"]} frames={frames} />);
    expect(row("a")).toBeNull();
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("list")));
    await act(() => new Promise((done) => requestAnimationFrame(() => setTimeout(done))));
    expect(frames.slice(from), "a draw let go of the hold").not.toContain(false);
    expect(row("c")).toBeNull();
  });

  it("lets go once the focus a taken-away row went with lands nowhere in the list", async () => {
    const frames: boolean[] = [];
    function Bare({ rows }: { rows: readonly string[] }) {
      const { holding, held } = useHeldStill<HTMLDivElement>(rows);
      useLayoutEffect(() => {
        frames.push(holding);
      });
      return (
        <div {...held}>
          {rows.map((row) => (
            <button key={row} type="button">
              {row}
            </button>
          ))}
        </div>
      );
    }
    const { rerender } = render(<Bare rows={["a", "b"]} />);
    act(() => button("a").focus());
    fireEvent.keyDown(button("a"), { key: "ArrowDown" });
    await waitFor(() => expect(frames.at(-1)).toBe(true));
    rerender(<Bare rows={["b"]} />);
    await waitFor(() => expect(frames.at(-1)).toBe(false));
  });
});
