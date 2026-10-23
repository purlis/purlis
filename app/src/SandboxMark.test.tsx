// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import { SandboxMark } from "./SandboxMark";
import { tipOf } from "./ui/tooltip/tooltip.testkit";

afterEach(cleanup);

/** #2071: a chat's sandbox level is always on its tab. */
describe("the sandbox mark", () => {
  it.each([
    ["full", "full sandbox"],
    ["floor", "floor"],
    ["none", "no sandbox"],
  ])("says the %s level by name", (level, said) => {
    const { container } = render(<SandboxMark level={level} />);
    const mark = container.querySelector(`[data-sandbox="${level}"]`);
    expect(mark).not.toBeNull();
    expect(tipOf(mark)).toEqual(said);
    expect(mark).toHaveAccessibleName(said);
    // The tab's text stays the chat's name alone.
    expect(mark).toHaveTextContent("");
  });

  it("says why a chat runs with no sandbox where the floor could not be applied (Q2)", () => {
    const why = "purlis runs Codex inside a sandbox of its own, which it can apply on macOS only";
    const { container } = render(<SandboxMark level="none" why={why} />);
    const mark = container.querySelector('[data-sandbox="none"]');
    expect(tipOf(mark)).toEqual(`no sandbox: ${why}`);
    expect(mark).toHaveAccessibleName(`no sandbox: ${why}`);
  });

  it("says how many sites only the chat's browser reaches, apart from its level (#2364)", () => {
    const { container, rerender } = render(<SandboxMark level="full" sites={2} />);
    const mark = container.querySelector('[data-sandbox="full"]');
    const said = "full sandbox; its browser reaches 2 sites of its own";
    expect(tipOf(mark)).toEqual(said);
    expect(mark).toHaveAccessibleName(said);
    rerender(<SandboxMark level="full" sites={1} of="The chat in front" />);
    expect(container.querySelector('[data-sandbox="full"]')).toHaveAccessibleName(
      "The chat in front: full sandbox; its browser reaches 1 site of its own",
    );
    rerender(<SandboxMark level="full" sites={0} />);
    expect(container.querySelector('[data-sandbox="full"]')).toHaveAccessibleName("full sandbox");
  });

  it("draws nothing for a shell or a chat still to start, nor for a word it does not know", () => {
    for (const level of [null, undefined, "partial"]) {
      const { container } = render(<SandboxMark level={level} />);
      expect(container.firstChild).toBeNull();
      cleanup();
    }
  });
});
