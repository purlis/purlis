// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { PATIENCE, onceTheKeyboardIsBack, theKeyboardIsBack, thePaletteIsUp } from "./keyboardBack";

afterEach(() => {
  theKeyboardIsBack();
  vi.useRealTimers();
});

describe("a keyboard move a palette row asks for (#1769)", () => {
  it("is made at once when no palette is up", () => {
    const move = vi.fn();
    onceTheKeyboardIsBack(move);
    expect(move).toHaveBeenCalledTimes(1);
  });

  it("waits while the palette is up, and is made once it has handed the keyboard back", () => {
    const move = vi.fn();
    thePaletteIsUp();
    onceTheKeyboardIsBack(move);
    expect(move).not.toHaveBeenCalled();

    theKeyboardIsBack();
    expect(move).toHaveBeenCalledTimes(1);
    theKeyboardIsBack();
    expect(move).toHaveBeenCalledTimes(1);
  });

  it("is made all the same when the palette never says it handed the keyboard back", () => {
    vi.useFakeTimers();
    const move = vi.fn();
    thePaletteIsUp();
    onceTheKeyboardIsBack(move);

    vi.advanceTimersByTime(PATIENCE - 1);
    expect(move).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(move).toHaveBeenCalledTimes(1);
  });
});
