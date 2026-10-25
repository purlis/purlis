// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { drawnWith } from "./cascade.testkit";

/** A pane's header with the harness chip as the button family draws it, in a split or alone. */
function harnessIn(pane: "alone" | "focused" | "unfocused"): Element {
  const chip = `<div class="pane-head"><div class="pane-chips"><button id="harness" class="pane-harness" data-scope="button" data-part="root" data-variant="ghost" data-size="sm">Claude Code</button></div></div>`;
  const frame = (focused: boolean) =>
    `<div class="pane-frame"${focused ? " data-focused" : ""}>${chip}</div>`;
  document.body.innerHTML =
    pane === "alone"
      ? frame(false)
      : `<div data-scope="panel" data-variant="content"><div data-group>${frame(pane === "focused")}</div></div>`;
  return document.getElementById("harness") as Element;
}

/**
 * **The harness's name keeps the header's tier** (#2914, #3129): muted on a pane alone and on the
 * pane of a split without the keyboard, primary on the one with it and under the pointer, over
 * the ghost variant's primary words.
 */
describe("the harness chip's words", () => {
  it("are muted on a lone pane and on a split's other pane", () => {
    expect(drawnWith(harnessIn("alone"), "color")).toBe("var(--text-muted)");
    expect(drawnWith(harnessIn("unfocused"), "color")).toBe("var(--text-muted)");
  });

  it("are primary on the pane with the keyboard, and under the pointer or the keyboard", () => {
    expect(drawnWith(harnessIn("focused"), "color")).toBe("var(--text-primary)");
    expect(drawnWith(harnessIn("alone"), "color", { hover: true })).toBe("var(--text-primary)");
    expect(drawnWith(harnessIn("unfocused"), "color", { focusVisible: true })).toBe(
      "var(--text-primary)",
    );
  });
});
