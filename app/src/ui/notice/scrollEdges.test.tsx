// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A pane's band says which way it has more to scroll** (#3168): in a narrow pane the band of
 * warnings can be taller than its room, and at rest nothing showed the buttons below. Its stack
 * marks the edge that has more past it (`data-more-above`, `data-more-below`), which its fade is
 * drawn from (`notice.css`), and no edge once there is nothing further.
 *
 * jsdom lays nothing out, so the stack's scroll geometry is set here as a browser would report
 * it; the fade itself is read in the real engines (`PaneBand.stories.tsx`, `NarrowScrolled`).
 */
import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import { NoticeStack } from "./Notice";

afterEach(cleanup);

/** Says the stack is `room` tall, holds `held`, and is scrolled `top` down, as layout would. */
function laidOut(stack: HTMLElement, { room, held, top }: { room: number; held: number; top: number }) {
  Object.defineProperty(stack, "clientHeight", { configurable: true, value: room });
  Object.defineProperty(stack, "scrollHeight", { configurable: true, value: held });
  Object.defineProperty(stack, "scrollTop", { configurable: true, writable: true, value: top });
}

function Band({ count = 2 }: { count?: number }) {
  return (
    <NoticeStack data-testid="stack">
      {Array.from({ length: count }, (_, at) => (
        <p key={at}>Warning {at + 1}</p>
      ))}
    </NoticeStack>
  );
}

const edges = (stack: HTMLElement) => ({
  above: stack.hasAttribute("data-more-above"),
  below: stack.hasAttribute("data-more-below"),
});

describe("a pane's band marks the edge it has more past (#3168)", () => {
  it("marks the bottom at rest while more is below, both partway, and the top at its end", () => {
    render(<Band />);
    const stack = screen.getByTestId("stack");
    laidOut(stack, { room: 120, held: 300, top: 0 });
    act(() => {
      stack.dispatchEvent(new Event("scroll"));
    });
    expect(edges(stack)).toEqual({ above: false, below: true });

    stack.scrollTop = 90;
    act(() => {
      stack.dispatchEvent(new Event("scroll"));
    });
    expect(edges(stack)).toEqual({ above: true, below: true });

    stack.scrollTop = 180;
    act(() => {
      stack.dispatchEvent(new Event("scroll"));
    });
    expect(edges(stack)).toEqual({ above: true, below: false });
  });

  it("marks no edge when everything fits, nor for the half pixel a scaled screen rounds to", () => {
    render(<Band />);
    const stack = screen.getByTestId("stack");
    laidOut(stack, { room: 120, held: 120.5, top: 0 });
    act(() => {
      stack.dispatchEvent(new Event("scroll"));
    });
    expect(edges(stack)).toEqual({ above: false, below: false });
  });

  it("looks again when what it holds changes, with no scroll", async () => {
    const { rerender } = render(<Band count={1} />);
    const stack = screen.getByTestId("stack");
    expect(edges(stack)).toEqual({ above: false, below: false });
    // A second warning arrives, and the band now holds more than its room.
    laidOut(stack, { room: 120, held: 260, top: 0 });
    rerender(<Band count={2} />);
    await act(async () => {
      await Promise.resolve();
    });
    expect(edges(stack)).toEqual({ above: false, below: true });
    // One is put behind "+N more" (`hidden`), and the rest fits again.
    laidOut(stack, { room: 120, held: 110, top: 0 });
    act(() => {
      stack.firstElementChild?.setAttribute("hidden", "");
    });
    await act(async () => {
      await Promise.resolve();
    });
    expect(edges(stack)).toEqual({ above: false, below: false });
  });
});
