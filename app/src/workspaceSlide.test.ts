// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { slideIn, slideTowards, takeSlide, wayOf } from "./workspaceSlide";

afterEach(() => {
  takeSlide();
  document.body.innerHTML = "";
});

describe("which way a workspace switch slides (R114)", () => {
  it("is the order's: on to a workspace after the old one, back to one before it", () => {
    const order = ["smart-ide", "charter", "skills", "docs"];
    expect(wayOf(order, "smart-ide", "skills")).toBe(1);
    expect(wayOf(order, "skills", "charter")).toBe(-1);
  });

  it("is a step's own when one asked, once", () => {
    slideTowards(1);
    expect(takeSlide()).toBe(1);
    expect(takeSlide()).toBeUndefined();
  });
});

describe("the slide itself (R114)", () => {
  it("marks the regions with the way, for the stylesheet to play, until it ends", () => {
    const parts = document.createElement("div");
    document.body.append(parts);
    slideIn(parts, -1);
    expect(parts.dataset.slide).toBe("back");
    // A switch during the slide marks it afresh.
    slideIn(parts, 1);
    expect(parts.dataset.slide).toBe("on");
    // What ends inside it is not its slide.
    const inner = document.createElement("span");
    parts.append(inner);
    inner.dispatchEvent(new Event("animationend", { bubbles: true }));
    expect(parts.dataset.slide).toBe("on");
    parts.dispatchEvent(new Event("animationend"));
    expect(parts.dataset.slide).toBeUndefined();
  });

  it("does nothing without the regions", () => {
    expect(() => slideIn(null, 1)).not.toThrow();
  });
});
