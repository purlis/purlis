// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import { FOCUS_BACK, FOCUS_MODE, useFocusMode } from "./focusMode";

/** A window as focus mode reads it: the rail, a side panel, and the work. */
function Window() {
  const focus = useFocusMode();
  return (
    <main
      data-testid="window"
      {...{ [FOCUS_MODE]: focus.on ? "" : undefined, [FOCUS_BACK]: focus.back ? "" : undefined }}
    >
      <div data-scope="rail" data-part="slot">
        <button type="button">A project</button>
      </div>
      <div className="region-slot slot-left">
        <button type="button">In the side</button>
      </div>
      <div className="region-centre">
        <button type="button">The work</button>
      </div>
      {/* The status bar's card (#3653): portaled over the window, and down with the bar. */}
      {!focus.on && (
        <div role="region" aria-label="Notices for one" data-chip="status/bar/notices">
          <button type="button">In the status card</button>
        </div>
      )}
      <button type="button" onClick={focus.toggle}>
        Toggle
      </button>
    </main>
  );
}

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("focus mode (N32)", () => {
  it("one press hides the window's parts, and the next brings them back, fading in once", () => {
    vi.useFakeTimers();
    render(<Window />);
    const window = screen.getByTestId("window");
    const toggle = screen.getByRole("button", { name: "Toggle" });
    expect(window).not.toHaveAttribute(FOCUS_MODE);
    act(() => toggle.click());
    expect(window).toHaveAttribute(FOCUS_MODE);
    expect(window).not.toHaveAttribute(FOCUS_BACK);
    act(() => toggle.click());
    expect(window).not.toHaveAttribute(FOCUS_MODE);
    // Coming back is marked for the fade, and only for as long as it runs.
    expect(window).toHaveAttribute(FOCUS_BACK);
    act(() => vi.advanceTimersByTime(1_000));
    expect(window).not.toHaveAttribute(FOCUS_BACK);
  });

  it("writes nothing: no store is touched", () => {
    const set = vi.spyOn(Storage.prototype, "setItem");
    render(<Window />);
    act(() => screen.getByRole("button", { name: "Toggle" }).click());
    expect(set).not.toHaveBeenCalled();
    set.mockRestore();
  });

  it("takes the keyboard from a part it hides to the work", () => {
    render(<Window />);
    const side = screen.getByRole("button", { name: "In the side" });
    side.focus();
    // The toggle as a key would press it, with the keyboard still in the side.
    act(() => screen.getByRole("button", { name: "Toggle" }).click());
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "The work" }));
  });

  it("takes the keyboard from the status bar's card, which goes with the bar, to the work", () => {
    // #3653's review: the card is no part of the footer, but goes as it goes; its Notices item,
    // hidden, takes no keyboard, so the keyboard would be left on nothing.
    render(<Window />);
    screen.getByRole("button", { name: "In the status card" }).focus();
    act(() => screen.getByRole("button", { name: "Toggle" }).click());
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "The work" }));
  });

  it("leaves the keyboard where it is when that stays drawn", () => {
    render(<Window />);
    const work = screen.getByRole("button", { name: "The work" });
    work.focus();
    act(() => screen.getByRole("button", { name: "Toggle" }).click());
    expect(document.activeElement).toBe(work);
  });
});
