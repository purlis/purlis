// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import { SandboxMark } from "./SandboxMark";

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
    expect(mark).toHaveAttribute("title", said);
    expect(mark).toHaveTextContent(said);
  });

  it("draws nothing for a shell or a chat still to start, nor for a word it does not know", () => {
    for (const level of [null, undefined, "partial"]) {
      const { container } = render(<SandboxMark level={level} />);
      expect(container.firstChild).toBeNull();
      cleanup();
    }
  });
});
