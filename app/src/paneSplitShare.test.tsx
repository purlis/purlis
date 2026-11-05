// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render } from "@testing-library/react";
import { EvenedSeparator } from "./LayoutPanes";

/**
 * **A split draws the share its tree holds** (#5291). The library reads a side's size once, as it
 * mounts, and a drop that leaves other panes on the same two sides keeps its group: without this
 * the window would draw the old sizes, where the preview promised the tree's.
 */

const group = vi.hoisted(() => ({
  getLayout: vi.fn(() => ({ a: 30, b: 70 })),
  setLayout: vi.fn(),
}));
vi.mock("react-resizable-panels", async (real) => ({
  ...(await real<typeof import("react-resizable-panels")>()),
  useParentGroup: () => group,
  Separator: () => <div />,
}));

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

const SIDES: [string, string] = ["a", "b"];

describe("a split's share (#5291)", () => {
  it("leaves the library's layout alone as it first draws", () => {
    render(<EvenedSeparator sides={SIDES} share={300} onEven={() => {}} />);
    expect(group.setLayout).not.toHaveBeenCalled();
  });

  it("takes the tree's share when a drop changes it under a live group", () => {
    const { rerender } = render(<EvenedSeparator sides={SIDES} share={300} onEven={() => {}} />);
    rerender(<EvenedSeparator sides={SIDES} share={500} onEven={() => {}} />);
    expect(group.setLayout).toHaveBeenCalledWith({ a: 50, b: 50 });
  });

  it("does nothing for a share the library already draws, as a separator's drag says it back", () => {
    const { rerender } = render(<EvenedSeparator sides={SIDES} share={500} onEven={() => {}} />);
    rerender(<EvenedSeparator sides={SIDES} share={300} onEven={() => {}} />);
    expect(group.setLayout).not.toHaveBeenCalled();
  });
});
