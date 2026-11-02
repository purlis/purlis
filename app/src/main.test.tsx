// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **The window's entry asks for a first frame's parts as it starts** (#4410): the frame a heavy
// theme was drawn without its parts in is handed to `partsLand`, or its parts land only when the
// restore asks what is in force. `partsLater.test.ts` holds what the ask does; this holds that
// the window makes it. The entry runs once per module graph, so this file loads it once.

import { mockIPC } from "@tauri-apps/api/mocks";
import { expect, it, vi } from "vitest";
import type { Frame } from "./windowprefs";

const landed = vi.hoisted(() => ({ frames: [] as Frame[] }));

// The window itself is not what is held here: `App` draws nothing, and what runs after the
// first frame (the layout's settling, the native window) answers at once.
vi.mock("./App", () => ({ default: () => null }));
vi.mock("./regions", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./regions")>()),
  settleLayout: () => Promise.resolve(),
}));
vi.mock("./theme/native", () => ({ drawNatively: () => undefined }));
vi.mock("./theme/partsLater", () => ({
  partsLand: (frame: Frame) => {
    landed.frames.push(frame);
    return Promise.resolve();
  },
}));
// The first frame a heavy theme's creation ran past the wait of: no parts, and a ticket to ask by.
vi.mock("./windowprefs", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./windowprefs")>();
  const { DEFAULT_THEME } = await import("./theme/theme");
  const heavy: Frame = {
    light: false,
    more: false,
    theme: { ...DEFAULT_THEME, name: "Heavy", parts: "" },
    floors: null,
    mine: [],
    parts_later: 3,
  };
  return { ...actual, firstFrameOnce: () => heavy };
});

it("hands the first frame drawn without its parts to be landed as the window starts", async () => {
  mockIPC(() => null);
  document.body.innerHTML = '<div id="root"></div>';

  await import("./main");

  expect(landed.frames).toEqual([expect.objectContaining({ parts_later: 3 })]);
  expect(landed.frames[0].theme.name).toBe("Heavy");
});
