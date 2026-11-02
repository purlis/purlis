// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, renderHook } from "@testing-library/react";
import { TOOLTIP_CLOSE_DELAY } from "../tooltip/Tooltip";
import { TIP_AFTER_MS, useRested } from "./MenuTip";

afterEach(() => {
  vi.useRealTimers();
  cleanup();
});

describe("a menu row's tooltip with the pointer on it", () => {
  it("goes when the pointer left the window, with no leave on the tooltip (#4628)", async () => {
    const { result } = renderHook(() => useRested(true));
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    // A key after the menu came is what makes a focus a person's rest.
    fireEvent.keyDown(window, { key: "ArrowDown" });
    act(() => result.current.rest.onFocus());
    act(() => void vi.advanceTimersByTime(TIP_AFTER_MS));
    expect(result.current.rested).toBe(true);

    // Off the row and onto the tooltip: it stays.
    act(() => {
      result.current.rest.onBlur();
      result.current.hold.onPointerEnter();
    });
    act(() => void vi.advanceTimersByTime(TOOLTIP_CLOSE_DELAY * 2));
    expect(result.current.rested).toBe(true);

    await act(async () => void fireEvent.pointerOut(document, { relatedTarget: null }));
    act(() => void vi.advanceTimersByTime(TOOLTIP_CLOSE_DELAY));

    expect(result.current.rested).toBe(false);
  });
});
