// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { TOOLTIP_DELAY } from "../tooltip/Tooltip";
import { useRowTip } from "./RowTip";

/** **A row's tooltip when the pointer leaves the window** with no leave on the row (#3095). */

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

function Row() {
  const { hover, drawn } = useRowTip("What the row does");
  return (
    <div data-testid="row" {...hover}>
      {drawn(false)}
    </div>
  );
}

describe("a row's tooltip, the pointer gone from the window (#3095)", () => {
  it("goes", () => {
    vi.useFakeTimers();
    render(<Row />);
    fireEvent.pointerEnter(screen.getByTestId("row"), { pointerType: "mouse" });
    act(() => void vi.advanceTimersByTime(TOOLTIP_DELAY));
    expect(screen.queryByRole("tooltip")).not.toBeNull();
    act(() => void fireEvent.pointerOut(document, { relatedTarget: null }));
    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  it("never comes, when the pointer left before its delay ran out", () => {
    vi.useFakeTimers();
    render(<Row />);
    fireEvent.pointerEnter(screen.getByTestId("row"), { pointerType: "mouse" });
    act(() => void fireEvent.pointerOut(document, { relatedTarget: null }));
    act(() => void vi.advanceTimersByTime(TOOLTIP_DELAY));
    expect(screen.queryByRole("tooltip")).toBeNull();
  });
});
