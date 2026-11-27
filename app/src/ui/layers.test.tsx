// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** **What stands over the window now** (#7728): the count of layers up, held whatever hears it. */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render } from "@testing-library/react";
import { hearLayers, layersUp, useLayerUp } from "./layers";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function Up() {
  useLayerUp("dialog", true);
  return null;
}

describe("the layers up in the window", () => {
  it("keeps its count, and tells the rest, when one that hears it throws", async () => {
    const thrown: unknown[] = [];
    vi.spyOn(window, "queueMicrotask").mockImplementation((run) => {
      try {
        run();
      } catch (error) {
        thrown.push(error);
      }
    });
    const after = vi.fn();
    const stopThrowing = hearLayers(() => {
      throw new Error("a listener broke");
    });
    const stopAfter = hearLayers(after);
    try {
      const { unmount } = render(<Up />);
      expect(layersUp("dialog")).toBe(1);
      unmount();
      expect(layersUp("dialog")).toBe(0);
      expect(after).toHaveBeenCalledTimes(2);
      expect(thrown).toHaveLength(2);
    } finally {
      stopThrowing();
      stopAfter();
    }
  });
});
