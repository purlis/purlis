// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { FitAddon } from "@xterm/addon-fit";
import { SessionPane } from "./SessionPane";
import { forgetTheSlide, slideEnded, slideStarted } from "./railSlide";

vi.mock("./renderer", async (real) => ({
  ...(await real<typeof import("./renderer")>()),
  draw: () => new Promise(() => {}),
}));
vi.stubGlobal("matchMedia", (query: string) => ({
  matches: false,
  media: query,
  addEventListener: () => {},
  removeEventListener: () => {},
  addListener: () => {},
  removeListener: () => {},
}));

/** Every resize observer the pane made, so a test can say its element changed size. */
const observers: (() => void)[] = [];

beforeEach(() => {
  observers.length = 0;
  forgetTheSlide();
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(private readonly heard: () => void) {
        observers.push(() => this.heard());
      }
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  );
  mockIPC(() => new Promise(() => {}));
});
afterEach(() => {
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
  forgetTheSlide();
});

/**
 * **A chat's terminal is fitted once per hide or show of the project rail** (#2915, B1): while the
 * rail's column slides, the pane is a little wider each frame, and a fit on each would tell the
 * program a new size each time (`resizeSession`). It waits until the slide is over.
 */
describe("a chat's terminal while the project rail slides", () => {
  it("fits once when the slide ends, however many frames it was resized for", () => {
    const fit = vi.spyOn(FitAddon.prototype, "fit").mockImplementation(() => {});
    render(<SessionPane plane="/planes/one" session={1} focused={false} onFocus={() => {}} />);
    expect(observers.length).toBeGreaterThan(0);
    const before = fit.mock.calls.length;

    slideStarted();
    // Sixteen frames of a 260 ms slide, each heard by the pane's observers.
    for (let frame = 0; frame < 16; frame++) for (const heard of observers) heard();
    expect(fit.mock.calls.length).toBe(before);

    slideEnded();
    expect(fit.mock.calls.length).toBe(before + 1);
  });

  it("fits nothing at the slide's end once the pane has gone", () => {
    const fit = vi.spyOn(FitAddon.prototype, "fit").mockImplementation(() => {});
    const { unmount } = render(
      <SessionPane plane="/planes/one" session={1} focused={false} onFocus={() => {}} />,
    );
    slideStarted();
    for (const heard of observers) heard();
    unmount();
    const before = fit.mock.calls.length;
    slideEnded();
    expect(fit.mock.calls.length).toBe(before);
  });

  it("fits at once when nothing slides", () => {
    const fit = vi.spyOn(FitAddon.prototype, "fit").mockImplementation(() => {});
    render(<SessionPane plane="/planes/one" session={1} focused={false} onFocus={() => {}} />);
    const before = fit.mock.calls.length;
    for (const heard of observers) heard();
    expect(fit.mock.calls.length).toBe(before + observers.length);
  });
});
