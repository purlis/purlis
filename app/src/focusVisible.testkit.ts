// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { onTestFinished, vi } from "vitest";

/**
 * **No focus is the keyboard's, for the rest of the test**: `:focus-visible` matches nothing, as
 * a real engine answers for the focus a pointer's press leaves on a row. jsdom cannot say it:
 * since 31 its matcher takes `:focus-visible` as `:focus`, so every focus reads as the
 * keyboard's. A test of what a pointer's focus does not do (the Chats list's hold, V100-47)
 * says the engine's answer with this, and it is undone when the test ends.
 */
export function focusIsNotTheKeyboards(): void {
  const matches = Element.prototype.matches;
  const spy = vi.spyOn(Element.prototype, "matches").mockImplementation(function (
    this: Element,
    selectors: string,
  ) {
    return selectors === ":focus-visible" ? false : matches.call(this, selectors);
  });
  onTestFinished(() => spy.mockRestore());
}
