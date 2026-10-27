// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { drawnWith } from "../../cascade.testkit";
import { Separator } from "./Separator";

/** **The separator family** (#2912): its own part, as thick as `separator.width`. */

afterEach(cleanup);

describe("Separator", () => {
  it("is a separator a screen reader hears, as thick as the theme's separator width", () => {
    render(<Separator />);
    const line = screen.getByRole("separator");
    expect(line).toHaveAttribute("data-scope", "separator");
    expect(line).toHaveAttribute("data-orientation", "horizontal");
    expect(drawnWith(line, "block-size")).toBe("var(--separator-width)");
    expect(drawnWith(line, "background-color")).toBe("var(--border-subtle)");
  });

  it("is hidden from a screen reader where it only parts two marks, and drawn down a strip", () => {
    const { container } = render(<Separator orientation="vertical" decorative />);
    expect(screen.queryByRole("separator")).toBeNull();
    const line = container.firstElementChild as Element;
    expect(line).toHaveAttribute("aria-hidden", "true");
    expect(drawnWith(line, "inline-size")).toBe("var(--separator-width)");
  });
});
