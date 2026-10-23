// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { drawNatively, forgetNativeWindow } from "./native";
import { PURLIS_LIGHT } from "./handed.testkit";

/** Every `draw_native_window` the window asks, by its arguments. */
function told(): Record<string, unknown>[] {
  const asked: Record<string, unknown>[] = [];
  mockIPC((cmd, args) => {
    if (cmd === "draw_native_window") asked.push((args ?? {}) as Record<string, unknown>);
    return null;
  });
  return asked;
}

afterEach(() => {
  clearMocks();
  forgetNativeWindow();
});

describe("the native window (#2328)", () => {
  it("is told the drawn theme once per change, and again when its window takes focus", async () => {
    const asked = told();
    const light = PURLIS_LIGHT;
    const once = {
      appearance: "light",
      background: light.values["window.background"],
      followsSystem: false,
    };

    drawNatively(light);
    drawNatively(light);
    await Promise.resolve();
    expect(asked).toEqual([once]);

    // The platform keeps one appearance for the app: the window in front says its own again.
    window.dispatchEvent(new Event("focus"));
    await Promise.resolve();
    expect(asked).toEqual([once, once]);

    drawNatively(light, true);
    await Promise.resolve();
    expect(asked.at(-1)).toEqual({ ...once, followsSystem: true });
  });
});
